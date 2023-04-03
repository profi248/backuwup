pub mod package;
pub mod packfile;
pub mod unpackage;

use std::ffi::OsString;

use serde::{Deserialize, Serialize};

pub const NONCE_SIZE: usize = 12;

pub type BlobHash = [u8; 32];
pub type PackfileId = [u8; 12];
pub type BlobNonce = [u8; NONCE_SIZE];

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Debug)]
pub enum BlobKind {
    FileChunk,
    Tree,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Debug)]
pub enum CompressionKind {
    None,
    Zstd,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Debug)]
pub enum TreeKind {
    File,
    Dir,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Debug)]
pub struct PackfileHeaderBlob {
    pub hash: BlobHash,
    pub kind: BlobKind,
    pub compression: CompressionKind,
    pub length: u64,
    pub offset: u64,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct Blob {
    pub hash: BlobHash,
    pub kind: BlobKind,
    pub data: Vec<u8>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct BlobEncrypted {
    pub hash: BlobHash,
    pub kind: BlobKind,
    pub data: Vec<u8>,
    pub nonce: BlobNonce,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Debug)]
struct Snapshot {
    id: u64,
    timestamp: u64,
    tree: BlobHash,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Debug, Default)]
struct TreeMetadata {
    size: Option<u64>,
    mtime: Option<u64>,
    ctime: Option<u64>,
}

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
