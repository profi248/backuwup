pub mod blob_index;
pub mod file_chunker;
pub mod packfile_handler;
pub mod walker;

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};

type BlobHash = [u8; 32];
type PackfileId = [u8; 12];

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
struct PackfileBlob {
    hash: BlobHash,
    kind: BlobKind,
    compression: CompressionKind,
    length: u64,
    offset: u64,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct Blob {
    pub hash: BlobHash,
    pub kind: BlobKind,
    pub data: Vec<u8>,
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

#[derive(Default, Debug)]
struct DirTreeMem {
    path: PathBuf,
    // todo fill in metadata properly
    metadata: TreeMetadata,
    children: Vec<Arc<Mutex<TreeFuture>>>,
}

#[derive(Debug)]
enum TreeFutureState {
    Unexplored(PathBuf),
    Explored(DirTreeMem),
    Completed(BlobHash),
}

#[derive(Debug)]
struct TreeFuture {
    data: TreeFutureState,
}

impl TreeFuture {
    fn new_path(path: impl Into<PathBuf>) -> Self {
        TreeFuture {
            data: TreeFutureState::Unexplored(path.into()),
        }
    }

    fn wrapped_new(path: impl Into<PathBuf>) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self::new_path(path)))
    }

    fn wrapped_new_visited(path: impl Into<PathBuf>) -> Arc<Mutex<Self>> {
        let future = Self {
            data: TreeFutureState::Explored(DirTreeMem::default()),
        };

        Arc::new(Mutex::new(future))
    }

    fn set_visited_with_path(&mut self, path: impl Into<PathBuf>) {
        self.data = TreeFutureState::Explored(DirTreeMem {
            path: path.into(),
            metadata: TreeMetadata::default(),
            children: vec![],
        });
    }

    fn unexplored_get_path(&self) -> &PathBuf {
        if let TreeFutureState::Unexplored(path) = &self.data {
            path
        } else {
            panic!("Invalid Unexplored tree future state");
        }
    }

    fn explored_get_tree(&mut self) -> &mut DirTreeMem {
        if let TreeFutureState::Explored(ref mut tree) = &mut self.data {
            tree
        } else {
            panic!("Invalid Explored tree future state");
        }
    }

    fn unwrap_explored(&self) -> &DirTreeMem {
        match &self.data {
            TreeFutureState::Explored(data) => data,
            _ => panic!("Unwrap explored failed, state is invalid"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PackfileError {
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
    #[error("Data decryption/encryption error")]
    CryptoError(#[from] aes_gcm::Error),
    #[error("{0}")]
    SerializationError(#[from] bincode::Error),
    #[error("{0}")]
    GetrandomError(#[from] getrandom::Error),
    #[error("Invalid Unicode string: {0:?}")]
    InvalidString(OsString),
}
