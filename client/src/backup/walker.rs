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

use crate::backup::{packfile_handler::PackfileHandler, Blob, BlobHash, BlobKind, DirTreeMem, Tree, TreeFuture, TreeKind, TreeMetadata, TreeFutureState};

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
                    iter = fs::read_dir(PathBuf::from(root_path).join(guard.unexplored_get_path()))?;
                    guard.deref_mut().data = TreeFutureState::Explored(DirTreeMem::default());
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

        let filename = path
            .file_name()
            .unwrap_or_else(|| "".as_ref())
            .to_string_lossy();

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
        let chunker = StreamCDC::new(source, 524_288, 2_097_152, 4_194_304);
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

        // todo split file trees that are too large

        let file_tree = bincode::serialize(&file_tree)?;
        let tree_hash = hash_bytes(&file_tree);

        packer
            .add_blob(Blob {
                hash: tree_hash,
                kind: BlobKind::Tree,
                data: file_tree,
            })
            .await?;

        let mut guard = future.lock().unwrap();
        guard.data = TreeFutureState::Completed(tree_hash);
    }

    println!("{root_tree:?}");

    // todo pack trees into chunks
    packer.flush().await?;

    Ok(())
}

fn hash_bytes(data: &[u8]) -> BlobHash {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}
