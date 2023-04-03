use std::{path::PathBuf, sync::atomic::Ordering};

use aes_gcm::{AeadInPlace, Aes256Gcm, KeyInit, Nonce};
use bincode::Options;
use tokio::{
    fs::{self, OpenOptions},
    io::AsyncWriteExt,
};
use zstd::bulk::Compressor;

use crate::{
    backup::{
        packfile::{
            Manager, KEY_DERIVATION_CONSTANT_HEADER, PACKFILE_MAX_BLOBS, PACKFILE_MAX_SIZE,
            PACKFILE_TARGET_SIZE, ZSTD_COMPRESSION_LEVEL,
        },
        Blob, BlobEncrypted, BlobNonce, CompressionKind, PackfileError, PackfileHeaderBlob,
        PackfileId, NONCE_SIZE,
    },
    defaults::BLOB_MAX_UNCOMPRESSED_SIZE,
    KEYS,
};

impl Manager {
    pub async fn add_blob(&self, blob: Blob) -> Result<(), PackfileError> {
        if blob.data.len() > BLOB_MAX_UNCOMPRESSED_SIZE {
            return Err(PackfileError::BlobTooLarge);
        }

        // deduplication: we won't queue a blob if has been queued already
        if self.inner.index.lock().await.is_blob_duplicate(&blob.hash) {
            return Ok(());
        }

        let (blob_data, nonce_bytes) = Self::compress_encrypt_blob(&blob)?;

        {
            self.inner.blobs.lock().await.push_back(BlobEncrypted {
                hash: blob.hash,
                kind: blob.kind,
                data: blob_data,
                nonce: nonce_bytes,
            });

            self.inner.dirty.store(true, Ordering::Relaxed);
        }

        self.trigger_write_if_desired().await?;
        Ok(())
    }

    fn compress_encrypt_blob(blob: &Blob) -> Result<(Vec<u8>, BlobNonce), PackfileError> {
        let mut compressor = Compressor::new(ZSTD_COMPRESSION_LEVEL)?;
        compressor.include_checksum(false)?;
        compressor.include_contentsize(false)?;
        compressor.include_magicbytes(false)?;

        let mut blob_data = compressor.compress(&blob.data)?;

        // derive a new key for each for each blob based on the (unencrypted) hash,
        // to ensure that we have a unique nonce/key combo
        let key = KEYS.get().unwrap().derive_backup_key(&blob.hash);
        let cipher = Aes256Gcm::new(&key.into());

        // generate a random nonce for each blob
        let mut nonce_bytes: BlobNonce = Default::default();
        getrandom::getrandom(&mut nonce_bytes)?;
        let nonce = Nonce::from_slice(&nonce_bytes);

        cipher.encrypt_in_place(nonce, b"", &mut blob_data)?;

        Ok((blob_data, nonce_bytes))
    }

    pub async fn flush(&self) -> Result<(), PackfileError> {
        self.write_packfiles().await?;

        self.inner.index.lock().await.flush().await?;
        self.inner.dirty.store(false, Ordering::Release);

        Ok(())
    }

    async fn trigger_write_if_desired(&self) -> Result<bool, PackfileError> {
        let mut candidates_size: usize = 0;
        let mut candidates_cnt: usize = 0;

        {
            let blobs = self.inner.blobs.lock().await;
            let mut index = self.inner.index.lock().await;

            for blob in blobs.iter() {
                if !index.is_blob_duplicate(&blob.hash) {
                    candidates_size += blob.data.len();
                    candidates_cnt += 1;
                }
            }
        }

        if candidates_size >= PACKFILE_TARGET_SIZE || candidates_cnt >= PACKFILE_MAX_BLOBS {
            return self.write_packfiles().await.map(|_| true);
        }

        Ok(false)
    }

    async fn write_packfiles(&self) -> Result<(), PackfileError> {
        let mut blobs = self.inner.blobs.lock().await;
        let mut index = self.inner.index.lock().await;

        while !blobs.is_empty() {
            let mut packfile_index = index.begin_packfile();
            let mut data: Vec<u8> = Vec::new();
            let mut header: Vec<PackfileHeaderBlob> = Vec::new();
            let mut blob_count: usize = 0;
            let mut bytes_written: usize = 0;

            while let Some(blob) = &mut blobs.pop_front() {
                // deduplication: double check that blob is unique
                if index.is_blob_duplicate(&blob.hash) {
                    continue;
                }

                // add blob to header
                header.push(PackfileHeaderBlob {
                    hash: blob.hash,
                    kind: blob.kind,
                    compression: CompressionKind::Zstd,
                    offset: bytes_written as u64,
                    length: blob.data.len() as u64,
                });

                bytes_written += blob.data.len() + NONCE_SIZE;

                // write blob to packfile buffer, as nonce[NONCE_SIZE] || encrypted_data[length]
                data.append(&mut blob.nonce.to_vec());
                data.append(&mut blob.data);

                index.add_to_packfile(&mut packfile_index, blob.hash)?;

                blob_count += 1;

                if bytes_written >= PACKFILE_TARGET_SIZE || blob_count >= PACKFILE_MAX_BLOBS {
                    break;
                }
            }

            // if no blobs were added to the packfile because of deduplication, skip writing it
            if blob_count == 0 {
                continue;
            }

            let (packfile_id, buffer) =
                Self::serialize_packfile(&mut data, &mut header, bytes_written)?;

            assert!(
                buffer.len() <= PACKFILE_MAX_SIZE,
                "bug: violated packfile size limit ({} B)",
                buffer.len()
            );

            let file_path = self.get_packfile_path(packfile_id, true).await?;

            // ensure that we are not overwriting an existing packfile by chance
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(file_path)
                .await?;

            // save the packfile data to disk and add it to index
            file.write_all(&buffer).await?;
            index.finalize_packfile(&packfile_index, packfile_id).await?;

            println!("wrote packfile {} of size {}", hex::encode(packfile_id), buffer.len());
        }

        Ok(())
    }

    fn serialize_packfile(
        mut data: &mut Vec<u8>,
        header: &mut Vec<PackfileHeaderBlob>,
        bytes_written: usize,
    ) -> Result<(PackfileId, Vec<u8>), PackfileError> {
        // generate a random packfile ID that will be used as a filename and a nonce for the header
        let mut packfile_id: PackfileId = Default::default();
        getrandom::getrandom(&mut packfile_id)?;

        // derive a key for headers based on a constant
        let key = KEYS
            .get()
            .unwrap()
            .derive_backup_key(KEY_DERIVATION_CONSTANT_HEADER);
        let cipher = Aes256Gcm::new(&key.into());

        // serialize and encrypt the header
        let mut header: Vec<u8> = bincode::options().with_varint_encoding().serialize(&header)?;
        cipher.encrypt_in_place(Nonce::from_slice(&packfile_id), b"", &mut header)?;

        let mut buffer: Vec<u8> =
            Vec::with_capacity(core::mem::size_of::<u64>() + header.len() + bytes_written);

        // create a packfile buffer with the following structure:
        // header_length[sizeof u64] || encrypted_header[header_length] || data
        buffer.append(&mut (header.len() as u64).to_le_bytes().to_vec());
        buffer.append(&mut header);
        buffer.append(&mut data);

        Ok((packfile_id, buffer))
    }

    pub async fn get_packfile_path(
        &self,
        packfile_hash: PackfileId,
        create_folders: bool,
    ) -> Result<PathBuf, PackfileError> {
        let packfile_hash_hex = hex::encode(packfile_hash);

        // split packfiles into directories based on the first two hex characters of the hash,
        // to avoid having too many files in the same directory
        let directory = self.inner.output_path.join(&packfile_hash_hex[..2]);
        let file_path = directory.join(packfile_hash_hex);

        if create_folders {
            fs::create_dir_all(directory).await?;
        };

        Ok(file_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::BlobKind;

    #[test]
    fn validate_size_constraints() {
        let entry = PackfileHeaderBlob {
            hash: [0; 32],
            kind: BlobKind::FileChunk,
            compression: CompressionKind::Zstd,
            offset: 0,
            length: 0,
        };

        let entry_len = bincode::options()
            .with_varint_encoding()
            .serialize(&entry)
            .unwrap()
            .len();

        // worst case scenario with maximum amount of blobs, target size reached and
        // a maximum size blob added over the target size
        assert!(
            PACKFILE_TARGET_SIZE
                + BLOB_MAX_UNCOMPRESSED_SIZE
                + (entry_len * PACKFILE_MAX_BLOBS)
                + NONCE_SIZE
                <= PACKFILE_MAX_SIZE
        );
    }
}
