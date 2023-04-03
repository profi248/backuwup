use std::{
    collections::VecDeque,
    ffi::OsString,
    fmt::{Debug, Formatter},
    fs,
    fs::File,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Instant,
};

use anyhow::bail;
use fastcdc::v2020::FastCDC;
use filetime::FileTime;
use futures_util::future::join_all;
use memmap2::Mmap;
use pathdiff::diff_paths;

use crate::{
    backup::{packfile, Blob, BlobHash, BlobKind, Tree, TreeKind, TreeMetadata},
    defaults::{BLOB_DESIRED_TARGET_SIZE, BLOB_MAX_UNCOMPRESSED_SIZE, BLOB_MINIMUM_TARGET_SIZE},
    LOGGER,
};

type FsNodePtr = Option<Arc<FsNode>>;

/// Maximum amount of children before it's split off to a sibling. This is to prevent the blob from
/// growing too big and exceeding the maximum size, as well as for effective deduplication.
const TREE_BLOB_MAX_CHILDREN: usize = 10_000;

struct FsNode {
    parent: FsNodePtr,
    name: OsString,
    children: Mutex<Vec<BlobHash>>,
}

#[allow(clippy::unused_async)]
pub async fn create(backup_root: PathBuf, pack_folder: PathBuf) -> anyhow::Result<BlobHash> {
    let packer = packfile::Manager::new(pack_folder).await?;

    let mut processing_queue = VecDeque::<FsNodePtr>::new();

    let root_node: FsNodePtr = Some(Arc::new(FsNode {
        parent: None,
        name: OsString::default(),
        children: Mutex::new(vec![]),
    }));

    processing_queue.push_back(root_node.clone());

    if backup_root.exists() {
        LOGGER.get().unwrap().send_backup_started();
    }

    let start = Instant::now();

    let mut total_file_count: u64 = 0;
    browse_dir_tree(
        &backup_root,
        root_node,
        &mut processing_queue,
        &mut total_file_count,
    )?;
    LOGGER.get().unwrap().progress_set_total(total_file_count);

    // todo flush here on error, and also implement index rebuilding
    // todo handle sigterm
    let root_hash =
        pack_files_in_directory(&backup_root, &mut processing_queue, packer.clone()).await?;
    packer.flush().await?;

    let elapsed = start.elapsed();
    LOGGER.get().unwrap().send_backup_finished(
        true,
        format!(
            "Backup finished successfully, processed {} files in {:02}:{:02}.",
            total_file_count,
            elapsed.as_secs() / 60,
            elapsed.as_secs() % 60,
        ),
    );

    Ok(root_hash)
}

/// Browse the provided directory, and fill the processing queue with directories,
/// so that we can process them in order they depend on each other. Deepest directories
/// need to be processed first, so we discover them and put them at the front of the queue.
fn browse_dir_tree(
    root_path: &PathBuf,
    root_node: FsNodePtr,
    processing_queue: &mut VecDeque<FsNodePtr>,
    total_file_count: &mut u64,
) -> anyhow::Result<()> {
    let mut browsing_queue = VecDeque::<(FsNodePtr, PathBuf)>::new();
    browsing_queue.push_back((root_node, root_path.clone()));

    while let Some((current_node, current_path)) = browsing_queue.pop_front() {
        let iter = fs::read_dir(root_path.join(current_path))?;
        for item in iter {
            match item {
                Ok(entry) => match entry.file_type() {
                    Ok(ftype) if ftype.is_dir() => {
                        let rel_path = diff_paths(entry.path(), root_path).unwrap_or(entry.path());
                        let node = Some(Arc::new(FsNode {
                            parent: current_node.clone(),
                            name: entry.path().file_name().unwrap_or("".as_ref()).to_owned(),
                            children: Mutex::new(vec![]),
                        }));

                        println!("found folder {rel_path:?}");

                        processing_queue.push_front(node.clone());
                        browsing_queue.push_front((node, rel_path));
                    }
                    Ok(ftype) if ftype.is_file() => {
                        *total_file_count += 1;
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
        let iter = fs::read_dir(get_node_path(root_path, node.clone()))?;

        // a rudimentary multithreading for the chunker, most of the other tasks are still single-threaded
        let mut futures = Vec::new();

        let mut dir_tree = Tree {
            kind: TreeKind::Dir,
            name: path.file_name().unwrap_or("".as_ref()).to_string_lossy().into(),
            metadata: get_metadata(&path)?,
            // move child directory hashes from temporary FsNode structure to our actual tree
            children: Vec::from(node.as_ref().unwrap().children.lock().unwrap().as_mut()),
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
                    }
                    Ok(_) => {
                        let logger = LOGGER.get().unwrap();

                        logger.progress_increment_failed();
                        logger.send(format!(
                            "file {} is neither a file or a directory, ignored",
                            entry.path().display()
                        ));
                    }
                    Err(e) => {
                        let logger = LOGGER.get().unwrap();

                        logger.progress_increment_failed();
                        logger.send(format!(
                            "error when scanning file {}: {e}, continuing",
                            entry.path().display()
                        ));
                    }
                },
                Err(e) => {
                    let logger = LOGGER.get().unwrap();

                    logger.progress_increment_failed();
                    logger.send(format!(
                        "error trying to discover files: {e}, continuing",
                    ));
                }
            }
        }

        // collect all of the processed file hashes into our directory tree
        let files = join_all(futures).await;
        for result in files {
            match result {
                Ok(Ok(hash)) => dir_tree.children.push(hash),
                Ok(Err(e)) => {
                    let logger = LOGGER.get().unwrap();
                    logger.progress_increment_failed();
                    logger.send(format!("error backing up a file: {e}"));
                },
                Err(e) => bail!("error processing backups: {e}"),
            }
        }

        let dir_blob_root_hash = add_tree_to_blobs(packer.clone(), &mut dir_tree).await?;

        // add our hash to our parent directory, unless we are the root
        // if we are the root, store our hash so we can use it as a snapshot
        match &node.as_ref().unwrap().parent {
            Some(parent) => parent.children.lock().unwrap().push(dir_blob_root_hash),
            None => root_tree_hash = dir_blob_root_hash,
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

fn get_metadata(path: &PathBuf) -> anyhow::Result<TreeMetadata> {
    let metadata = path.metadata()?;

    Ok(TreeMetadata {
        size: Some(metadata.len()),
        mtime: match metadata.modified().map(|t| FileTime::from(t).unix_seconds()) {
            Ok(ts @ 0..) => Some(ts as u64),
            Ok(_) => None,
            Err(_) => None,
        },
        ctime: match metadata.created().map(|t| FileTime::from(t).unix_seconds()) {
            Ok(ts @ 0..) => Some(ts as u64),
            Ok(_) => None,
            Err(_) => None,
        },
    })
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
    let filename = match path.file_name() {
        Some(f) => f,
        None => bail!("unable to get file name at {path:?}"),
    };

    let mut file_tree = Tree {
        kind: TreeKind::File,
        name: filename.to_string_lossy().into(),
        metadata: get_metadata(&path)?,
        children: Vec::default(),
        next_sibling: None,
    };

    // split file into chunks if it's large
    if fs::metadata(path.clone())?.len() > BLOB_DESIRED_TARGET_SIZE as u64 {
        let file = File::open(path.clone())?;

        // safety: the worst that could happen here if data gets modified while mmap'd, is that the
        // resulting backup would be wrong. while that's bad, it's something that's hard to avoid in
        // general. we can try to also store file hashes to prevent errors like this, or try using locks
        let mmap = unsafe { Mmap::map(&file)? };

        let chunker = FastCDC::new(
            &mmap,
            BLOB_MINIMUM_TARGET_SIZE as u32,
            BLOB_DESIRED_TARGET_SIZE as u32,
            BLOB_MAX_UNCOMPRESSED_SIZE as u32,
        );

        for chunk in chunker {
            let data = &mmap[chunk.offset..(chunk.offset + chunk.length)];
            let hash = blake3::hash(data).into();

            file_tree.children.push(hash);

            packer
                .add_blob(Blob {
                    hash,
                    kind: BlobKind::FileChunk,
                    data: Vec::from(data),
                })
                .await?;
        }
    } else {
        let blob = fs::read(path.clone())?;
        let hash = blake3::hash(&blob).into();
        file_tree.children.push(hash);

        packer
            .add_blob(Blob {
                hash,
                kind: BlobKind::FileChunk,
                data: blob,
            })
            .await?;
    }

    let tree_blobs = split_serialize_tree(&file_tree)?;
    let first_blob_hash = tree_blobs[0].hash;

    for blob in tree_blobs {
        packer.add_blob(blob).await?;
    }

    LOGGER
        .get()
        .unwrap()
        .progress_notify_increment(path.to_string_lossy().to_string());

    Ok(first_blob_hash)
}

fn split_serialize_tree(tree: &Tree) -> anyhow::Result<VecDeque<Blob>> {
    // at this point, we could sort the vector of children for directory blobs

    if tree.children.len() <= TREE_BLOB_MAX_CHILDREN {
        let data = bincode::serialize(&tree)?;
        let vec = vec![Blob {
            hash: blake3::hash(&data).into(),
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
                hash: blake3::hash(&data).into(),
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
