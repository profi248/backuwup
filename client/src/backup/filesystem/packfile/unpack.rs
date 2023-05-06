//! Contains the logic for unpacking blobs from packfiles.

use aes_gcm::{AeadInPlace, Aes256Gcm, KeyInit, Nonce};
use bincode::Options;
use shared::types::{BlobHash, NONCE_SIZE};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncSeekExt},
};
use zstd::bulk::Decompressor;

use crate::{
    backup::filesystem::{
        packfile::{Manager, KEY_DERIVATION_CONSTANT_HEADER, PACKFILE_MAX_SIZE},
        Blob, PackfileError, PackfileHeaderBlob,
    },
    defaults::BLOB_MAX_UNCOMPRESSED_SIZE,
    KEYS,
};

impl Manager {
    /// Returns the blob if it exists in the packfile.
    pub async fn get_blob(&mut self, blob_hash: &BlobHash) -> Result<Option<Blob>, PackfileError> {
        if let Some(packfile_id) = self.inner.index.lock().await.find_packfile(blob_hash) {
            let path = self.get_packfile_path(packfile_id, false).await?;
            let mut packfile = File::open(path).await?;
            let packfile_size = packfile.metadata().await?.len();
            if packfile_size > PACKFILE_MAX_SIZE as u64 {
                return Err(PackfileError::PackfileTooLarge);
            }

            let mut header_size_bytes: [u8; core::mem::size_of::<u64>()] = Default::default();
            packfile.read_exact(&mut header_size_bytes).await?;
            let header_size = u64::from_le_bytes(header_size_bytes);

            if header_size > packfile_size || header_size == 0 {
                return Err(PackfileError::InvalidHeaderSize);
            }

            let mut header_buf = vec![0; header_size as usize];
            packfile.read_exact(&mut header_buf).await?;

            let key = KEYS.get().unwrap().derive_backup_key(KEY_DERIVATION_CONSTANT_HEADER);
            let cipher = Aes256Gcm::new(&key.into());
            cipher.decrypt_in_place(Nonce::from_slice(&packfile_id), b"", &mut header_buf)?;

            let header: Vec<PackfileHeaderBlob> =
                bincode::options().with_varint_encoding().deserialize(&header_buf)?;

            for blob_metadata in header {
                if blob_metadata.hash == *blob_hash {
                    let mut blob_nonce = [0; NONCE_SIZE];
                    let mut blob_buf = vec![0; blob_metadata.length as usize];
                    packfile
                        .seek(std::io::SeekFrom::Current(blob_metadata.offset as i64))
                        .await?;

                    packfile.read_exact(&mut blob_nonce).await?;
                    packfile.read_exact(&mut blob_buf).await?;

                    let key = KEYS.get().unwrap().derive_backup_key(&blob_metadata.hash);
                    let cipher = Aes256Gcm::new(&key.into());
                    cipher.decrypt_in_place(Nonce::from_slice(&blob_nonce), b"", &mut blob_buf)?;

                    let mut decompressor = Decompressor::new()?;
                    decompressor.include_magicbytes(false)?;
                    let blob_data = decompressor.decompress(&blob_buf, BLOB_MAX_UNCOMPRESSED_SIZE)?;

                    return Ok(Some(Blob {
                        hash: blob_metadata.hash,
                        kind: blob_metadata.kind,
                        data: blob_data,
                    }));
                }
            }

            Err(PackfileError::IndexHeaderMismatch)
        } else {
            // todo handle index not having the blob better
            println!("blob not found in index!!");
            Ok(None)
        }
    }
}
