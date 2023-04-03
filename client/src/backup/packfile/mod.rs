pub mod blob_index;
pub mod pack;
pub mod unpack;

use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use tokio::sync::Mutex;

use crate::backup::{packfile::blob_index::BlobIndex, BlobEncrypted, PackfileError};

/// Total blob size, after which it's attempted to write the packfile to disk.
pub const PACKFILE_TARGET_SIZE: usize = 3 * 1024 * 1024; // 3 MiB
/// Maximum possible size of a packfile.
pub const PACKFILE_MAX_SIZE: usize = 16 * 1024 * 1024; // 16 MiB
/// Maximum number of blobs that can be stored in a packfile.
pub const PACKFILE_MAX_BLOBS: usize = 100_000;

const ZSTD_COMPRESSION_LEVEL: i32 = 3;
const KEY_DERIVATION_CONSTANT_HEADER: &[u8] = b"header";

const PACKFILE_FOLDER: &str = "pack";
const INDEX_FOLDER: &str = "index";

/// A struct used for writing and reading packfiles, a file format used for storing blobs efficiently
/// and securely. Packfiles can contain one or more blobs, and are useful for preventing the
/// existence of many small loose files, and instead pack those small files together so all
/// packfiles are around the same size. PackfileHandler is also responsible for deduplicating blobs
/// so identical data is only stored once. Along with the packfiles, an index is also stored in
/// the output folder to allow for quick seeking.
///
/// When creating a backup, PackfileHandler takes in blobs, compresses and encrypts them and saves
/// them to packfiles. A reverse operation is done when restoring a backup. Packfiles are stored in
/// a folder named "pack" in the output folder, and the index is stored in a folder named "index"
/// in the output folder.
///
/// The format of a packfile is as follows:
/// - header length [8 bytes]
/// - encrypted header (encoded with bincode)
///     - (for each blob):
///         - blob hash (of unencrypted, uncompressed data)
///         - blob kind (file or directory)
///         - blob compression type (currently only zstd)
///         - blob data length (after encryption)
///         - blob data offset (from start of blob section, including nonce)
/// - individually encrypted blob data
///     - (for each blob):
///         - nonce [12 bytes]
///         - actual encrypted blob data
///
/// The header is encrypted with a key derived from the master key and a constant. Packfile ID is
/// random and used as a nonce for encrypting the header. Each blob is encrypted with a key derived
/// from the master key and the specific blob hash, nonce is random and stored along with the
/// encrypted data. All encryption/authentication is done using AES-256-GCM. Header length is
/// currently not encrypted, so it's possible to estimate the number of blobs stored in a file,
/// but I don't think it's a big problem now.
///

#[derive(Clone)]
pub struct Manager {
    inner: Arc<PackfileHandlerInner>,
}

struct PackfileHandlerInner {
    // Blobs in queue to be written to packfile.
    blobs: Mutex<VecDeque<BlobEncrypted>>,
    /// Keeps track if data has been successfully flushed to disk.
    dirty: AtomicBool,
    /// Index struct managing blob => packfile mapping.
    index: Mutex<BlobIndex>,
    /// The path to the output folder.
    output_path: PathBuf,
}

impl Drop for PackfileHandlerInner {
    fn drop(&mut self) {
        if self.dirty.load(Ordering::Acquire) {
            panic!("Packer was dropped while dirty, without calling flush()");
        }
    }
}

impl Manager {
    pub async fn new(output_path: PathBuf) -> Result<Self, PackfileError> {
        let packfile_path = output_path.join(PACKFILE_FOLDER);
        let index_path = output_path.join(INDEX_FOLDER);

        Ok(Self {
            inner: Arc::new(PackfileHandlerInner {
                blobs: Mutex::new(VecDeque::new()),
                output_path: packfile_path,
                index: Mutex::new(BlobIndex::new(index_path).await?),
                dirty: AtomicBool::new(false),
            }),
        })
    }

    pub async fn dump_index(&self) {
        self.inner.index.lock().await.dump().await;
    }
}
