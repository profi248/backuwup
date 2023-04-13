use std::{collections::VecDeque, path::PathBuf};

use anyhow::bail;
use filetime::{set_file_mtime, FileTime};
use futures_util::future::join_all;
use shared::types::BlobHash;
use tokio::{fs, fs::File, io::AsyncWriteExt};

use crate::backup::filesystem::{packfile, BlobKind, Tree, TreeKind};

pub async fn unpack(
    packfile_dir: impl Into<PathBuf>,
    destination_dir: impl Into<PathBuf> + Clone,
    root_hash: BlobHash,
) -> anyhow::Result<()> {
    let packer = packfile::Manager::new(packfile_dir.into()).await?;

    let destination_dir = destination_dir.into();
    fs::create_dir_all(destination_dir.clone()).await?;

    let root_tree = fetch_full_tree(packer.clone(), &root_hash).await?;

    let mut dir_queue: VecDeque<(Tree, PathBuf)> = VecDeque::new();
    dir_queue.push_front((root_tree, PathBuf::new()));

    while let Some((parent_tree, path)) = dir_queue.pop_front() {
        // all children of dir type tree are trees, all children of file type tree are chunks
        match parent_tree.kind {
            TreeKind::Dir => {
                let mut futures = Vec::new();

                for hash in &parent_tree.children {
                    let child_tree = fetch_full_tree(packer.clone(), hash).await?;
                    let rel_path = path.join(child_tree.name.clone());
                    let abs_path = destination_dir.join(&rel_path);

                    match child_tree.kind {
                        TreeKind::File => {
                            futures.push(tokio::spawn(restore_file(
                                packer.clone(),
                                Box::new(child_tree),
                                abs_path,
                            )));
                        }
                        TreeKind::Dir => {
                            fs::create_dir_all(&abs_path).await?;
                            set_path_mtime(&abs_path, &child_tree)?;
                            dir_queue.push_back((child_tree, rel_path));
                        }
                    }
                }

                // parallelize by directories for now, waiting for all files can use a lot of memory
                let results = join_all(futures).await;
                for result in results {
                    match result {
                        Ok(Ok(_)) => {}
                        Ok(Err(e)) => println!("error restoring file: {e:?}"),
                        Err(e) => println!("error occurred when restoring backups: {e:?}"),
                    }
                }
            }
            // files are processed directly
            TreeKind::File => {}
        }
    }

    println!("restoration done!!");

    Ok(())
}

async fn restore_file(
    mut packer: packfile::Manager,
    child_tree: Box<Tree>,
    path: PathBuf,
) -> anyhow::Result<()> {
    println!("restoring file {path:?}");

    let mut file = File::create(path.clone()).await?;
    for blob_hash in &child_tree.children {
        match packer.get_blob(blob_hash).await? {
            Some(blob) => file.write_all(&blob.data).await?,
            None => bail!("Blob {} not found", hex::encode(blob_hash)),
        }
    }

    set_path_mtime(&path, &child_tree)?;
    Ok(())
}

fn set_path_mtime(path: &PathBuf, tree: &Tree) -> anyhow::Result<()> {
    if let Some(time) = tree
        .metadata
        .mtime
        .map(|t| FileTime::from_unix_time(t as i64, 0))
    {
        set_file_mtime(path, time)?;
    }

    Ok(())
}

async fn fetch_full_tree(packer: packfile::Manager, hash: &BlobHash) -> anyhow::Result<Tree> {
    let mut root_tree = fetch_tree(packer.clone(), hash).await?;

    let mut next_hash = root_tree.next_sibling;
    while let Some(hash) = &next_hash {
        let mut partial_tree = fetch_tree(packer.clone(), hash).await?;
        root_tree.children.append(&mut partial_tree.children);
        next_hash = partial_tree.next_sibling;
    }

    Ok(root_tree)
}

async fn fetch_tree(mut packer: packfile::Manager, hash: &BlobHash) -> anyhow::Result<Tree> {
    let tree_blob = match packer.get_blob(hash).await? {
        Some(t) => t,
        None => bail!(format!("Chunk {} was not found", hex::encode(hash))),
    };

    if tree_blob.kind != BlobKind::Tree {
        bail!(format!("Chunk {} is not a tree", hex::encode(hash)))
    }

    let tree: Tree = bincode::deserialize(&tree_blob.data)?;
    Ok(tree)
}
