use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::anyhow;
use shared::types::PackfileId;

use crate::defaults::{INDEX_FOLDER, PACKFILE_FOLDER};

/// Parse a packfile path from a file name into a native packfile ID.
pub fn parse_packfile_path_into_id(path: &Path) -> anyhow::Result<PackfileId> {
    let packfile_id: PackfileId = hex::decode(
        path.file_name()
            .ok_or(anyhow!("can't get packfile filename"))?
            .to_string_lossy()
            .to_string(),
    )?
    .try_into()
    .map_err(|_| anyhow!("invalid packfile filename"))?;

    Ok(packfile_id)
}

/// Parse an index path from a file name into a native index ID.
pub fn parse_index_path_into_id(path: &Path) -> anyhow::Result<u32> {
    let index_num = path
        .file_name()
        .ok_or(anyhow!("cannot get index filename"))?
        .to_string_lossy()
        .to_string()
        .parse()?;

    Ok(index_num)
}

/// Get the path to a packfile, optionally creating parent directories if they don't exist.
pub fn get_packfile_path(backup_folder: &Path, id: PackfileId, create_dirs: bool) -> anyhow::Result<PathBuf> {
    let mut path = backup_folder.join(PACKFILE_FOLDER);
    let hex = hex::encode(id);

    // save the packfile in a folder named after the first two bytes of the hash
    path.push(&hex[..2]);
    if create_dirs {
        fs::create_dir_all(&path)?;
    };

    path.push(hex);
    Ok(path)
}

/// Get the path to an index file.
pub fn get_index_path(backup_folder: &Path, id: u32) -> PathBuf {
    backup_folder.join(INDEX_FOLDER).join(format!("{id:0>10}"))
}
