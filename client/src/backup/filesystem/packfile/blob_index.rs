//! Contains the index implementation, which is used for quickly finding packfiles.

use std::{collections::HashSet, path::PathBuf};

use aes_gcm::{AeadInPlace, Aes256Gcm, KeyInit, Nonce};
use bincode::Options;
use shared::types::{BlobHash, PackfileId};
use tokio::{
    fs::{self, File},
    io::{AsyncReadExt, AsyncWriteExt},
};
use tokio_stream::{wrappers::ReadDirStream, StreamExt};

use crate::{backup::filesystem::PackfileError, KEYS};

const MAX_FILE_ENTRIES: usize = 50_000;

const NONCE_SIZE: usize = 12;
const KEY_DERIVATION_CONSTANT: &[u8] = b"index";

type Entry = Vec<(BlobHash, PackfileId)>;

/// Index is a set of files containing mappings of blob => packfile.
/// It is useful for quickly finding the packfile to fetch if we want a particular blob.
/// The source of truth for data stored in packfiles are their headers, index
/// is just a tool for making seeking easier. Index can always be reconstructed
/// from packfile headers, but is primarily written along with newly created packfiles.
///
/// On disk, index consists of many files in a folder, using a sequential numbering system.
/// To load the index in memory, all files are combined into one long list. The reason for splitting
/// index into individual files is to make additions easy, and to prevent files from growing too large.
/// The index files are also encrypted before saving, with a key derived from the backup primary key and a
/// constant, and the index ID as nonce. Index capacity is capped to 50 000 entries, making the largest file
/// size slightly larger than 2 MiB.
///
/// Idea to improve space efficiency of index (if needed):
/// To save space, we'll only store the minimum amount required to uniquely find a blob.
/// At first only the initial 3 bytes will be saved, if a collision is found, we will store as
/// many bytes as necessary. When reading index, the longest matching blob hash will be
/// treated as the correct entry. Best case scenario hash will only take up 4 bytes
/// (3 bytes of hash + 1 byte of length) instead of 32 bytes. Size per index entry is
/// 4-33 + 12 bytes, or 20-45 in total. For 2 500 000 stored blobs (that is about 5 TB of data
/// or 2 500 000 individual files), total index size would range from about 50 to 112 megabytes.
pub struct BlobIndex {
    /// Path to the index folder.
    output_path: PathBuf,
    /// Index entries loaded from disk.
    items: Entry,
    /// Index entries waiting to be written to disk.
    items_buf: Entry,
    /// All blob hashes that have been queued for writing or have already been written.
    blobs_queued: HashSet<BlobHash>,
    /// Numeric ID of the last written index file.
    last_file_num: u32,
    /// Keeps track if index has been successfully flushed to disk.
    dirty: bool,
}

pub struct IndexPackfileHandle {
    /// Blobs contained in the a currently constructed packfile.
    blobs: Vec<BlobHash>,
}

impl BlobIndex {
    /// Initializes an index, creating all necessary folders and loading existing index files.
    pub async fn new(output_path: PathBuf) -> Result<Self, PackfileError> {
        fs::create_dir_all(&output_path).await?;
        let mut index_files = ReadDirStream::new(fs::read_dir(&output_path).await?);

        let mut max_num = 0;
        while let Some(entry) = index_files.next().await {
            // ignore files that don't match our pattern
            max_num = max_num.max(
                (entry?
                    .file_name()
                    .into_string()
                    .map_err(PackfileError::InvalidString)?)
                .parse::<u32>()
                .unwrap_or(0),
            );
        }

        let mut index = Self {
            output_path,
            items: Default::default(),
            items_buf: Default::default(),
            blobs_queued: Default::default(),
            last_file_num: max_num,
            dirty: false,
        };

        index.load().await?;
        Ok(index)
    }

    /// Returns a handle to a newly created packfile for index.
    pub fn begin_packfile(&mut self) -> IndexPackfileHandle {
        IndexPackfileHandle { blobs: Vec::default() }
    }

    /// Adds a blob to the packfile in index.
    pub fn add_to_packfile(
        &mut self,
        handle: &mut IndexPackfileHandle,
        blob_hash: BlobHash,
    ) -> Result<(), PackfileError> {
        handle.blobs.push(blob_hash);

        if !self.blobs_queued.insert(blob_hash) {
            return Err(PackfileError::DuplicateBlob);
        }

        Ok(())
    }

    /// Finalizes a packfile in index, actually adding all blobs to the final index.
    pub async fn finalize_packfile(
        &mut self,
        handle: &IndexPackfileHandle,
        packfile_hash: PackfileId,
    ) -> Result<(), PackfileError> {
        for blob_hash in &handle.blobs {
            self.push(blob_hash, &packfile_hash).await?;
        }

        Ok(())
    }

    /// Returns whether the blob is already known by the index.
    pub fn is_blob_duplicate(&mut self, blob_hash: &BlobHash) -> bool {
        if self.blobs_queued.contains(blob_hash) {
            return true;
        }

        if self.find_packfile(blob_hash).is_some() {
            return true;
        }

        false
    }

    /// Finds the packfile that contains the blob.
    pub fn find_packfile(&self, blob_hash: &BlobHash) -> Option<PackfileId> {
        match self.items.binary_search_by_key(&blob_hash, |(a, _)| a) {
            Ok(entry_idx) => Some(self.items[entry_idx].1),
            Err(_) => None,
        }
    }

    /// Adds a mapping from blob hash to packfile hash to the index, flushing to disk if over threshold.
    pub async fn push(
        &mut self,
        blob_hash: &BlobHash,
        packfile_hash: &PackfileId,
    ) -> Result<(), PackfileError> {
        self.items_buf.push((*blob_hash, *packfile_hash));
        self.dirty = true;

        if self.items_buf.len() >= MAX_FILE_ENTRIES {
            self.flush().await?;
        }

        Ok(())
    }

    /// Loads all index files from disk.
    async fn load(&mut self) -> Result<(), PackfileError> {
        let mut index_files = ReadDirStream::new(fs::read_dir(&self.output_path).await?);

        while let Some(entry) = index_files.next().await {
            let entry = entry?;
            // ignore files that don't match our pattern
            let file_num = (entry
                .file_name()
                .into_string()
                .map_err(PackfileError::InvalidString)?)
            .parse::<u32>();
            if let Ok(file_num) = file_num {
                let mut file = File::open(entry.path()).await?;
                let mut buf: Vec<u8> = Vec::default();
                file.read_to_end(&mut buf).await?;

                let key = KEYS.get().unwrap().derive_backup_key(KEY_DERIVATION_CONSTANT);
                let cipher = Aes256Gcm::new(&key.into());
                let nonce_bytes = self.counter_to_nonce(file_num);
                let nonce = Nonce::from_slice(&nonce_bytes);

                // associated data could be used
                cipher.decrypt_in_place(nonce, b"", &mut buf)?;

                let mut items: Entry = bincode::options().with_varint_encoding().deserialize(&buf)?;
                self.items.append(&mut items);
            }
        }

        // sort all the entries so we're able to use binary search
        self.items.sort_unstable_by_key(|&(a, _)| a);

        Ok(())
    }

    /// Unconditionally flushes the index to disk.
    pub async fn flush(&mut self) -> Result<(), PackfileError> {
        let mut buf = bincode::options().with_varint_encoding().serialize(&self.items_buf)?;
        let new_file_num = self
            .last_file_num
            .checked_add(1)
            .expect("bug: index file counter overflow");

        // derive a key for index and let nonce be the index file number
        let key = KEYS.get().unwrap().derive_backup_key(KEY_DERIVATION_CONSTANT);
        let cipher = Aes256Gcm::new(&key.into());
        let nonce_bytes = self.counter_to_nonce(new_file_num);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // associated data could be used (to make sure we're decrypting a index so "index" could work)
        cipher.encrypt_in_place(nonce, b"", &mut buf)?;

        let file_name = format!("{new_file_num:0>10}");
        let file_path = self.output_path.join(file_name);
        let mut file = File::create(file_path.clone()).await?;
        file.write_all(&buf).await?;

        self.last_file_num = new_file_num;
        self.items_buf.clear();
        self.dirty = false;

        Ok(())
    }

    /// Converts a file number to a nonce.
    fn counter_to_nonce(&self, file_number: u32) -> [u8; NONCE_SIZE] {
        let mut nonce_bytes = [0; NONCE_SIZE];
        nonce_bytes[0..4].copy_from_slice(&file_number.to_le_bytes());

        nonce_bytes
    }
}

impl Drop for BlobIndex {
    fn drop(&mut self) {
        if self.dirty {
            panic!("Index was dropped while dirty, without calling flush()");
        }
    }
}
