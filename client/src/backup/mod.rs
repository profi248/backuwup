use std::path::PathBuf;

use anyhow::bail;
use fs_extra::dir::get_size;
use futures_util::{try_join, FutureExt};
use orchestrator::Orchestrator;
use tokio::sync::OnceCell;

use crate::{backup::filesystem::dir_packer, CONFIG, UI};
use crate::net_server::requests;

pub mod filesystem;
pub mod orchestrator;
pub mod restore_send;
pub mod send;

pub static BACKUP_ORCHESTRATOR: OnceCell<Orchestrator> = OnceCell::const_new();

pub async fn run() -> anyhow::Result<()> {
    let config = CONFIG.get().unwrap();
    let destination = config.get_packfile_path().await?;

    Orchestrator::initialize_static(&destination).await?;

    let backup_path = config.get_backup_path().await?;
    if backup_path.is_none() {
        bail!("backup path not set")
    }

    BACKUP_ORCHESTRATOR.get().unwrap().set_backup_started();

    // create a size estimate to use with storage requests
    estimate_size(backup_path.as_ref().unwrap());

    // start tasks for filesystem walking and for sending to peers
    let pack_result = tokio::spawn(dir_packer::pack(backup_path.unwrap(), destination.clone()));
    let transport_result = tokio::spawn(send::send(destination));

    // unpack the inner result so it stops whenever one of the tasks returns an error
    let result = try_join!(pack_result.map(|r| r.unwrap()), transport_result.map(|r| r.unwrap()));

    // todo if packer fails, send might keep running
    BACKUP_ORCHESTRATOR.get().unwrap().set_backup_finished();

    match result {
        Ok((hash, _)) => {
            // inform the server that the backup is done
            requests::backup_done(hash).await?;

            UI.get()
                .unwrap()
                .send_backup_finished(true, "Backup completed successfully!");

            UI.get()
                .unwrap()
                .log(format!("Backup completed successfully! Snapshot hash: {}", hex::encode(hash)));
        }
        Err(e) => {
            UI.get()
                .unwrap()
                .send_backup_finished(false, format!("Backup failed: {e:?}"));
        }
    };

    Ok(())
}

pub async fn request_restore() -> anyhow::Result<()> {
    // retrieve the snapshot id and contacted peers from the server
    // request all files from all peers
    // restore files to backup path

    Ok(())
}

fn estimate_size(backup_path: &PathBuf) {
    match get_size(backup_path) {
        Ok(size) => {
            // use a constant to estimate compression of a typical dataset,
            // we could do something smarter based on file types, but this is good enough for now
            let size = (size as f64 * 0.9) as u64;
            BACKUP_ORCHESTRATOR.get().unwrap().set_size_estimate(size);
        }
        Err(e) => {
            BACKUP_ORCHESTRATOR.get().unwrap().set_backup_finished();

            UI.get()
                .unwrap()
                .send_backup_finished(false, format!("Backup failed: {e:?}"));
        }
    }
}

#[macro_export]
macro_rules! block_if_paused {
    () => {
        let orchestrator = $crate::backup::BACKUP_ORCHESTRATOR.get().unwrap();
        if !orchestrator.should_continue() {
            // block the backup until we can continue
            orchestrator.subscribe().await.await.unwrap();
        }
    };
}
