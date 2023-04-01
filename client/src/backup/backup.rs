use std::{
    cell::RefCell,
    collections::VecDeque,
    ffi::OsString,
    fmt::{Debug, Formatter},
    fs,
    fs::{File, Metadata},
    path::PathBuf,
    rc::Rc,
    time::UNIX_EPOCH,
};

use fastcdc::v2020::StreamCDC;
use futures_util::future::join_all;
use pathdiff::diff_paths;
use sha2::{Digest, Sha256};

use crate::{
    backup::{packfile, Blob, BlobHash, BlobKind, Tree, TreeKind, TreeMetadata},
    defaults::{BLOB_DESIRED_TARGET_SIZE, BLOB_MAX_UNCOMPRESSED_SIZE, BLOB_MINIMUM_TARGET_SIZE},
};

type FsNodePtr = Option<Rc<FsNode>>;

/// Maximum amount of children before it's split off to a sibling. This is to prevent the blob from
/// growing too big and exceeding the maximum size, as well as for effective deduplication.
const TREE_BLOB_MAX_CHILDREN: usize = 10_000;

struct FsNode {
    parent: FsNodePtr,
    name: OsString,
    children: RefCell<Vec<BlobHash>>,
}

#[allow(clippy::unused_async)]
pub async fn walk(
    backup_root: impl Into<PathBuf> + Clone,
    pack_folder: impl Into<String>,
) -> anyhow::Result<BlobHash> {
    let packer = packfile::Manager::new(pack_folder.into()).await?;

    let mut processing_queue = VecDeque::<FsNodePtr>::new();

    let root_node: FsNodePtr = Some(Rc::new(FsNode {
        parent: None,
        name: Default::default(),
        children: RefCell::new(vec![]),
    }));

    processing_queue.push_back(root_node.clone());

    println!("scanning folders...");

    browse_dir_tree(&backup_root.clone().into(), root_node, &mut processing_queue)?;
    println!("{processing_queue:?}");

    let root_hash = pack_files_in_directory(&backup_root.into(), &mut processing_queue, packer.clone()).await?;
    packer.flush().await?;

    Ok(root_hash)
}

/// Browse the provided directory, and fill the processing queue with directories,
/// so that we can process them in order they depend on each other. Deepest directories
/// need to be processed first, so we discover them and put them at the front of the queue.
fn browse_dir_tree(
    root_path: &PathBuf,
    root_node: FsNodePtr,
    processing_queue: &mut VecDeque<FsNodePtr>,
) -> anyhow::Result<()> {
    let mut browsing_queue = VecDeque::<(FsNodePtr, PathBuf)>::new();
    browsing_queue.push_back((root_node, root_path.clone()));

    while let Some((current_node, current_path)) = browsing_queue.pop_front() {
        let iter = fs::read_dir(root_path.join(current_path))?;
        for item in iter {
            match item {
                Ok(entry) => match entry.file_type() {
                    Ok(ftype) if ftype.is_dir() => {
                        let rel_path = diff_paths(entry.path(), &root_path).unwrap_or(entry.path());
                        let node = Some(Rc::new(FsNode {
                            parent: current_node.clone(),
                            name: entry.path().file_name().unwrap_or("".as_ref()).to_owned(),
                            children: RefCell::new(vec![]),
                        }));

                        println!("found folder {rel_path:?}");

                        processing_queue.push_front(node.clone());
                        browsing_queue.push_front((node, rel_path));
                    }
                    Ok(ftype) if ftype.is_file() => {}
                    Ok(_) => {
                        println!(
                            "file {} is neither a file or a directory, ignored",
                            entry.path().display()
                        );
                    }
                    Err(e) => {
                        println!(
                            "error when scanning file {}: {e}, continuing",
                            entry.path().display()
                        );
                    }
                },
                Err(e) => {
                    println!("error when scanning files: {e}, continuing");
                }
            }
        }
    }

    Ok(())
}

/// Take in the queue of directories, and pack files in directories in that order, so we can build
/// the final tree of chunks. This function passes directory trees, file trees and file data itself
/// to the packer.
async fn pack_files_in_directory(
    root_path: &PathBuf,
    processing_queue: &mut VecDeque<FsNodePtr>,
    packer: packfile::Manager,
) -> anyhow::Result<BlobHash> {
    let mut root_tree_hash = Default::default();

    while let Some(node) = processing_queue.pop_front() {
        let path = get_node_path(root_path, node.clone());
        println!("{:?}", get_node_path(root_path, node.clone()));
        let iter = fs::read_dir(get_node_path(root_path, node.clone()))?;

        // a rudimentary multithreading for the chunker, most of the other tasks are still single-threaded
        let mut futures = Vec::new();

        let mut dir_tree = Tree {
            kind: TreeKind::Dir,
            name: path.file_name().unwrap_or("".as_ref()).to_string_lossy().into(),
            metadata: get_metadata(path.metadata()?),
            // move child directory hashes from temporary FsNode structure to our actual tree
            children: Vec::from(node.as_ref().unwrap().children.borrow_mut().as_mut()),
            next_sibling: None,
        };

        for item in iter {
            match item {
                Ok(entry) => match entry.file_type() {
                    Ok(ftype) if ftype.is_file() => {
                        // start processing all the files and collect hashes later
                        futures.push(tokio::spawn(process_file(entry.path(), packer.clone())));
                    }
                    Ok(ftype) if ftype.is_dir() => {
                        // directory hashes have already been added to FsNode by their children
                        println!("discovered directory {entry:?}");
                    }
                    Ok(_) => {
                        println!(
                            "file {} is neither a file or a directory, ignored",
                            entry.path().display()
                        );
                    }
                    Err(e) => {
                        println!(
                            "error when scanning file {}: {e}, continuing",
                            entry.path().display()
                        );
                    }
                },
                Err(e) => {
                    println!("error when scanning files: {e}, continuing");
                }
            }
        }

        // collect all of the processed file hashes into our directory tree
        let files = join_all(futures).await;
        for result in files {
            match result {
                Ok(Ok(hash)) => dir_tree.children.push(hash),
                Ok(Err(e)) => println!("error backing up a file: {e}"),
                Err(e) => println!("error when processing backups: {e}"),
            }
        }

        println!("tree!! {dir_tree:?}");

        let dir_blob_root_hash = add_tree_to_blobs(packer.clone(), &mut dir_tree).await?;

        // add our hash to our parent directory, unless we are the root
        // if we are the root, store our hash so we can use it as a snapshot
        match &node.as_ref().unwrap().parent {
            Some(parent) => parent.children.borrow_mut().push(dir_blob_root_hash),
            None => root_tree_hash = dir_blob_root_hash
        }
    }

    Ok(root_tree_hash)
}

async fn add_tree_to_blobs(
    packer: packfile::Manager,
    dir_tree: &mut Tree,
) -> anyhow::Result<BlobHash> {
    let tree_blobs = split_serialize_tree(dir_tree)?;
    let first_blob_hash = tree_blobs[0].hash;

    for blob in tree_blobs {
        packer.add_blob(blob).await?;
    }

    Ok(first_blob_hash)
}

fn get_metadata(metadata: Metadata) -> TreeMetadata {
    TreeMetadata {
        size: Some(metadata.len()),
        mtime: match metadata.modified() {
            Ok(systime) => match systime.duration_since(UNIX_EPOCH) {
                Ok(ts) => Some(ts.as_secs()),
                Err(_) => None,
            },
            Err(_) => None,
        },
        ctime: match metadata.created() {
            Ok(systime) => match systime.duration_since(UNIX_EPOCH) {
                Ok(ts) => Some(ts.as_secs()),
                Err(_) => None,
            },
            Err(_) => None,
        },
    }
}

fn get_node_path(root_path: &PathBuf, mut node: FsNodePtr) -> PathBuf {
    let mut components = Vec::new();
    let mut path = root_path.clone();

    while node.is_some() {
        components.push(node.as_ref().as_ref().unwrap().name.clone());
        node = node.as_ref().as_ref().unwrap().parent.clone();
    }

    for str in components.iter().rev() {
        path.push(str);
    }

    path
}

async fn process_file(path: PathBuf, packer: packfile::Manager) -> anyhow::Result<BlobHash> {
    let filename = path.file_name().unwrap_or_else(|| "".as_ref()).to_string_lossy();

    let mut file_tree = Tree {
        kind: TreeKind::File,
        name: filename.clone().into(),
        metadata: get_metadata(path.metadata()?),
        children: Vec::default(),
        next_sibling: None,
    };

    let source = File::open(path.clone())?;

    let chunker = StreamCDC::new(
        source,
        BLOB_MINIMUM_TARGET_SIZE as u32,
        BLOB_DESIRED_TARGET_SIZE as u32,
        BLOB_MAX_UNCOMPRESSED_SIZE as u32,
    );

    for result in chunker {
        let chunk = result?;
        let hash = hash_bytes(&chunk.data);

        file_tree.children.push(hash);

        packer
            .add_blob(Blob {
                hash,
                kind: BlobKind::FileChunk,
                data: chunk.data,
            })
            .await?;

        println!(
            "path={:?} offset={} length={} hash={}",
            &path,
            chunk.offset,
            chunk.length,
            hex::encode(hash)
        );
    }

    println!("{file_tree:?}");

    let tree_blobs = split_serialize_tree(&file_tree)?;
    let first_blob_hash = tree_blobs[0].hash;

    for blob in tree_blobs {
        packer.add_blob(blob).await?;
    }

    Ok(first_blob_hash)
}

fn hash_bytes(data: &[u8]) -> BlobHash {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

fn split_serialize_tree(tree: &Tree) -> anyhow::Result<VecDeque<Blob>> {
    // at this point, we could sort the vector of children for directory blobs

    if tree.children.len() <= TREE_BLOB_MAX_CHILDREN {
        let data = bincode::serialize(&tree)?;
        let vec = vec![Blob {
            hash: hash_bytes(&data),
            kind: BlobKind::Tree,
            data,
        }];

        Ok(VecDeque::from(vec))
    } else {
        let mut split_tree = Vec::default();
        let mut split_serialized = VecDeque::<Blob>::default();

        // first we need to split the children into batches of at most `TREE_BLOB_MAX_CHILDREN`
        for chunk in tree.children.chunks(TREE_BLOB_MAX_CHILDREN) {
            split_tree.push(Tree {
                kind: tree.kind,
                name: tree.name.clone(),
                metadata: tree.metadata,
                children: Vec::from(chunk),
                next_sibling: None,
            });
        }

        // afterwards we always need to set next_sibling of the previous tree to the hash of
        // the next one, so we'll reverse the vector and serialize from the end,
        // filling in the hashes of previous blobs as we go back
        for (idx, mut tree) in split_tree.into_iter().rev().enumerate() {
            // for other trees than the very last one, take the hash of the previous tree
            // -- first element, because we push to front, so children are in order
            if idx != 0 {
                tree.next_sibling = Some(split_serialized[0].hash);
            }

            let data = bincode::serialize(&tree)?;
            let blob = Blob {
                hash: hash_bytes(&data),
                kind: BlobKind::Tree,
                data,
            };

            split_serialized.push_front(blob);
        }

        Ok(split_serialized)
    }
}

impl Debug for FsNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let name = &self.name.to_string_lossy().to_string();
        let parent = if self.parent.is_some() {
            format!("\"{}\"", self.parent.as_ref().as_ref().unwrap().name.to_string_lossy())
        } else {
            "None".to_string()
        };

        write!(f, "FsNode {{ name: \"{name}\", parent*: {parent} }}")
    }
}
