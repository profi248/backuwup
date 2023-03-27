use std::{
    collections::VecDeque,
    fs,
    fs::{File, ReadDir},
    ops::DerefMut,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use fastcdc::v2020::StreamCDC;
use pathdiff::diff_paths;
use sha2::{Digest, Sha256};

/// Maximum amount of children before it's split off to a sibling. This is to prevent the blob from
/// growing too big and exceeding the maximum size, as well as for effective deduplication.
const TREE_BLOB_MAX_CHILDREN: usize = 10_000;

use crate::{
    backup::{
        packfile_handler::PackfileHandler, Blob, BlobHash, BlobKind, DirTreeMem, Tree, TreeFuture,
        TreeFutureState, TreeKind, TreeMetadata,
    },
    defaults::{BLOB_DESIRED_TARGET_SIZE, BLOB_MAX_UNCOMPRESSED_SIZE, BLOB_MINIMUM_TARGET_SIZE},
};

pub async fn walk() -> anyhow::Result<()> {
    let mut packer = PackfileHandler::new("/home/david/_packs".to_string())
        .await
        .unwrap();

    let root_path = "/home/david/FIT/bachelors-thesis/backup-test";

    let root_tree = TreeFuture::wrapped_new_visited(root_path);
    let mut curr_tree = root_tree.clone();

    let mut iter: ReadDir;
    let mut dir_queue = VecDeque::<Arc<Mutex<TreeFuture>>>::new();
    let mut file_queue = VecDeque::<Arc<Mutex<TreeFuture>>>::new();

    iter = fs::read_dir(root_path)?;

    // iterate over directories recursively, building a tree skeleton and and filling queues
    loop {
        for item in &mut iter {
            match item {
                Ok(entry) => match entry.file_type() {
                    Ok(ftype) if ftype.is_dir() => {
                        let mut guard = curr_tree.lock().unwrap();
                        let tree_data = guard.explored_get_tree();

                        tree_data.children.push(TreeFuture::wrapped_new(
                            diff_paths(entry.path(), root_path).unwrap_or(entry.path()),
                        ));
                        dir_queue.push_back(tree_data.children.last().unwrap().clone());

                        println!("dir: {}", entry.path().display());
                    }
                    Ok(ftype) if ftype.is_file() => {
                        let mut guard = curr_tree.lock().unwrap();
                        let tree_data = guard.explored_get_tree();

                        tree_data.children.push(TreeFuture::wrapped_new(
                            diff_paths(entry.path(), root_path).unwrap_or(entry.path()),
                        ));
                        file_queue.push_back(tree_data.children.last().unwrap().clone());
                        println!("file: {}", entry.path().display());
                    }
                    Ok(ftype) if ftype.is_symlink() => {
                        // todo deal with symlinks
                        println!("symlink: {}", entry.path().display());
                    }
                    _ => {
                        println!("couldn't get file type: {}", entry.path().display());
                    }
                },
                Err(err) => println!("ERROR: {}", err),
            }
        }

        match dir_queue.pop_front() {
            Some(tree) => {
                {
                    let mut guard = tree.lock().unwrap();
                    let rel_path = guard.unexplored_get_path().clone();
                    iter =
                        fs::read_dir(PathBuf::from(root_path).join(rel_path.clone()))?;
                    guard.deref_mut().set_visited_with_path(rel_path);
                }

                curr_tree = tree;
            }
            None => break,
        }
    }

    println!("{root_tree:?}");
    println!("{file_queue:?}");

    while let Some(future) = file_queue.pop_front() {
        let path = {
            let guard = future.lock().unwrap();
            guard.unexplored_get_path().clone()
        };

        let filename = path.file_name().unwrap_or_else(|| "".as_ref()).to_string_lossy();

        let mut file_tree = Tree {
            kind: TreeKind::File,
            name: filename.clone().into(),
            metadata: TreeMetadata {
                size: None,
                mtime: None,
                ctime: None,
            },
            children: Vec::default(),
            next_sibling: None,
        };

        let source = File::open(PathBuf::from(root_path).join(&path)).unwrap();
        let chunker = StreamCDC::new(
            source,
            BLOB_MINIMUM_TARGET_SIZE as u32,
            BLOB_DESIRED_TARGET_SIZE as u32,
            BLOB_MAX_UNCOMPRESSED_SIZE as u32,
        );
        for result in chunker {
            let chunk = result.unwrap();
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

        let mut guard = future.lock().unwrap();
        guard.data = TreeFutureState::Completed(first_blob_hash);
    }

    println!("{root_tree:?}");

    let mut guard = root_tree.lock().unwrap();
    //let mut stack = Vec::new();
    loop {
        for child in &guard.unwrap_explored().children {
            println!("{child:?}");
        }
        break;
    }

    packer.flush().await?;

    Ok(())
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
                data
            };

            split_serialized.push_front(blob);
        }

        Ok(split_serialized)
    }
}
