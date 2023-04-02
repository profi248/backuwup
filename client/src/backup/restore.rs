use std::{collections::VecDeque, fs, fs::File, io::Write, path::PathBuf};

use anyhow::bail;
use futures_util::future::join_all;

use crate::backup::{packfile, packfile::Manager, BlobHash, BlobKind, Tree, TreeKind};

pub async fn unpack(
    packfile_dir: impl Into<String>,
    destination_dir: impl Into<PathBuf> + Clone,
    root_hash: BlobHash,
) -> anyhow::Result<()> {
    let packer = packfile::Manager::new(packfile_dir.into()).await?;
    packer.dump_index().await;

    let destination_dir = destination_dir.into();

    fs::create_dir_all(destination_dir.clone())?;
    let root_tree = fetch_full_tree(packer.clone(), &root_hash).await?;

    //let mut dir_stack = vec![Box::new(root_tree.clone())];
    let mut dir_queue: VecDeque<(Tree, PathBuf)> = VecDeque::new();

    dir_queue.push_front((root_tree, PathBuf::new()));

    let mut futures = Vec::new();

    // todo restore metadata
    while let Some((parent_tree, path)) = dir_queue.pop_front() {
        // all children of dir type tree are trees, all children of file type tree are chunks
        match parent_tree.kind {
            TreeKind::Dir => {
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
                            fs::create_dir_all(&abs_path)?;
                            dir_queue.push_back((child_tree, rel_path));
                        }
                    }
                }
            }
            // files are processed directly
            TreeKind::File => {}
        }
    }

    let results = join_all(futures).await;

    for result in results {
        match result {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => println!("error restoring file: {e:?}"),
            Err(e) => println!("error occurred when restoring backups: {e:?}"),
        }
    }

    println!("restoration done!!");

    Ok(())
}

async fn restore_file(
    mut packer: Manager,
    child_tree: Box<Tree>,
    path: PathBuf,
) -> anyhow::Result<()> {
    println!("restoring file {path:?}");

    let mut file = File::create(path)?;
    for blob_hash in &child_tree.children {
        match packer.get_blob(blob_hash).await? {
            Some(blob) => file.write_all(&blob.data)?,
            None => bail!("Blob {} not found", hex::encode(blob_hash)),
        }
    }

    Ok(())
}

fn get_path(stack: &[Box<Tree>], mut root_path: PathBuf) -> PathBuf {
    for tree in stack {
        root_path.push(&tree.name);
    }

    root_path
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
