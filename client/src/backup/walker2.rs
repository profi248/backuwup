// idea: push items (files/directories) on an unified queue, in order in which they need to be processed

use std::cell::RefCell;
use std::collections::{LinkedList, VecDeque};
use std::ffi::{OsString};
use std::fmt::{Debug, Formatter};
use std::path::{PathBuf};
use std::fs;
use std::rc::Rc;
use pathdiff::diff_paths;

type FsNodePtr = Rc<Option<FsNode>>;

#[allow(clippy::unused_async)]
pub async fn walk(root_path: PathBuf) -> anyhow::Result<()> {
    // maybe queue for browsing, queue for processing
    let mut processing_queue = VecDeque::<FsNodePtr>::new();
    // small queue for current folder where files get put at the end, and folders at the front, this gets put into big queue

    let root_node: FsNodePtr = Rc::new(Some(FsNode {
        parent: Rc::new(None),
        name: Default::default(),
    }));

    processing_queue.push_back(Rc::clone(&root_node));

    println!("scanning folders...");

    browse_dir_tree(&root_path, root_node, &mut processing_queue)?;
    println!("{processing_queue:?}");

    pack_files_in_directory(&root_path, &mut processing_queue)?;

    Ok(())
}

/// Browse the provided directory, and fill the processing queue with directories,
/// so that we can process them in order they depend on each other. Deepest directories
/// need to be processed first, so we discover them and put them at the front of the queue.
fn browse_dir_tree(root_path: &PathBuf, root_node: FsNodePtr, processing_queue: &mut VecDeque<FsNodePtr>) -> anyhow::Result<()> {
    let mut browsing_queue = VecDeque::<(FsNodePtr, PathBuf)>::new();
    browsing_queue.push_back((root_node, root_path.clone()));

    while let Some((current_node, current_path)) = browsing_queue.pop_front() {
        let iter = fs::read_dir(root_path.join(current_path))?;
        for item in iter {
            match item {
                Ok(entry) => match entry.file_type() {
                    Ok(ftype) if ftype.is_dir() => {
                        let rel_path = diff_paths(entry.path(), &root_path).unwrap_or(entry.path());
                        let node = Rc::new(Some(FsNode {
                            parent: Rc::clone(&current_node),
                            name: entry.path().file_name().unwrap_or("".as_ref()).to_owned(),
                        }));

                        println!("found folder {rel_path:?}");

                        processing_queue.push_front(Rc::clone(&node));
                        browsing_queue.push_front((node, rel_path));
                    },
                    Ok(ftype) if ftype.is_file() => {},
                    Ok(_) => { println!("file {} is neither a file or a directory, ignored", entry.path().display()) }
                    Err(e) => { println!("error when scanning file {}: {e}, continuing", entry.path().display()) }
                },
                Err(e) => { println!("error when scanning files: {e}, continuing") }
            }
        }
    }

    Ok(())
}

fn pack_files_in_directory(root_path: &PathBuf, processing_queue: &mut VecDeque<FsNodePtr>) -> anyhow::Result<()> {
    while let Some(node) = processing_queue.pop_front() {
        println!("{:?}", get_node_path(root_path, node.clone()));
        let iter = fs::read_dir(get_node_path(root_path, node))?;
        for item in iter {
            // maybe we can do async parallel files processing here
            match item {
                Ok(entry) => match entry.file_type() {
                    Ok(ftype) if ftype.is_dir() => {
                        // we will have to process those too, and pick out their hashes
                    },
                    Ok(ftype) if ftype.is_file() => {
                        // todo: iterate over all the files in the folder and chunk them
                        // after that, the folder can be chunked too and we can move on to other folders in the queue
                        // maybe we'll need a cell in the bitchass FsNode struct to save the final hash
                        // actually no, we'll save the hash of the folder to its parent by the pointer
                    },
                    Ok(_) => { println!("file {} is neither a file or a directory, ignored", entry.path().display()) }
                    Err(e) => { println!("error when scanning file {}: {e}, continuing", entry.path().display()) }
                },
                Err(e) => { println!("error when scanning files: {e}, continuing") }
            }
        }
    }

    Ok(())
}

fn get_node_path(root_path: &PathBuf, mut node: FsNodePtr) -> PathBuf {
    let mut components = Vec::new();
    let mut path = root_path.clone();

    while node.is_some() {
        components.push(node.as_ref().as_ref().unwrap().name.clone());
        node = Rc::clone(&node.as_ref().as_ref().unwrap().parent);
    }

    for str in components.iter().rev() {
        path.push(str);
    }

    path
}

struct FsNode {
    parent: FsNodePtr,
    name: OsString
}

impl Debug for FsNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let name = &self.name.to_string_lossy().to_string();
        let parent = if self.parent.is_some() {
            format!("\"{}\"", self.parent.as_ref().as_ref().unwrap().name.to_string_lossy().to_string())
        } else {
            "None".to_string()
        };

        write!(f, "FsNode {{ name: \"{name}\", parent*: {parent} }}")
    }
}
