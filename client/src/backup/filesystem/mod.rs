pub mod dir_packer;
pub mod dir_unpacker;
pub mod file_utils;
pub mod packfile;

use std::ffi::OsString;

use serde::{Deserialize, Serialize};
use shared::types::{BlobHash, BlobNonce};

/// Represents the type of the blob, either a file chunk or a tree.
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Debug)]
pub enum BlobKind {
    FileChunk,
    Tree,
}

// Specifies the compression algorithm used for the blob.
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Debug)]
pub enum CompressionKind {
    None,
    Zstd,
}

// Specifies the type of the tree, either a file or a directory.
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Debug)]
pub enum TreeKind {
    File,
    Dir,
}

/// Represents an item in the header of a packfile (a single blob).
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Debug)]
pub struct PackfileHeaderBlob {
    pub hash: BlobHash,
    pub kind: BlobKind,
    pub compression: CompressionKind,
    pub length: u64,
    pub offset: u64,
}

/// Represents an in-memory blob.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct Blob {
    pub hash: BlobHash,
    pub kind: BlobKind,
    pub data: Vec<u8>,
}

/// Represents an in-memory encrypted blob.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct BlobEncrypted {
    pub hash: BlobHash,
    pub kind: BlobKind,
    pub data: Vec<u8>,
    pub nonce: BlobNonce,
}

/// Represents metadata of a file/directory.
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Debug, Default)]
struct TreeMetadata {
    size: Option<u64>,
    mtime: Option<u64>,
    ctime: Option<u64>,
}

/// Represents a directory tree, for encoding into a blob or in-memory.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
struct Tree {
    kind: TreeKind,
    name: String,
    metadata: TreeMetadata,
    children: Vec<BlobHash>,
    next_sibling: Option<BlobHash>,
}

#[derive(Debug, thiserror::Error)]
pub enum PackfileError {
    #[error("Packfile local buffer over limit")]
    ExceededBufferLimit,
    #[error("Invalid packfile header size")]
    InvalidHeaderSize,
    #[error("Packfile too large")]
    PackfileTooLarge,
    #[error("Blob found in index, but not in packfile. Index might be out of date")]
    IndexHeaderMismatch,
    #[error("Blob too large")]
    BlobTooLarge,
    #[error("Duplicate blob in packfile index")]
    DuplicateBlob,
    #[error("{0}")]
    IoError(#[from] std::io::Error),
    #[error("{0}")]
    FsError(#[from] fs_extra::error::Error),
    #[error("Data decryption/encryption error")]
    CryptoError(#[from] aes_gcm::Error),
    #[error("{0}")]
    SerializationError(#[from] bincode::Error),
    #[error("{0}")]
    GetrandomError(#[from] getrandom::Error),
    #[error("Invalid Unicode string: {0:?}")]
    InvalidString(OsString),
}
