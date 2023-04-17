use std::path::PathBuf;

use anyhow::{anyhow, bail};
use backup_orchestrator::BackupOrchestrator;
use fs_extra::dir::get_size;
use futures_util::{try_join, FutureExt};
use shared::{p2p_message::RequestType, server_message::BackupRestoreInfo, types::ClientId};
use tokio::{sync::OnceCell, time::sleep};

use crate::{
    backup::{
        filesystem::{dir_packer, dir_unpacker},
        restore_orchestrator::RestoreOrchestrator,
    },
    log,
    net_server::requests,
    CONFIG, TRANSPORT_REQUESTS, UI,
};

pub mod backup_orchestrator;
pub mod filesystem;
pub mod restore_orchestrator;
pub mod restore_send;
pub mod send;

pub static BACKUP_ORCHESTRATOR: OnceCell<BackupOrchestrator> = OnceCell::const_new();
pub static RESTORE_ORCHESTRATOR: OnceCell<RestoreOrchestrator> = OnceCell::const_new();

pub async fn run() -> anyhow::Result<()> {
    let config = CONFIG.get().unwrap();
    let destination = config.get_packfile_path().await?;

    BackupOrchestrator::initialize_static(&destination).await?;

    if let Some(restore_orchestrator) = RESTORE_ORCHESTRATOR.get() {
        if restore_orchestrator.is_running() {
            bail!("cannot backup, restore already in progress");
        }
    }

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
    RestoreOrchestrator::initialize_static().await?;
    RESTORE_ORCHESTRATOR.get().unwrap().set_started()?;
    return match run_restore().await {
        Ok(_) => Ok(()),
        Err(e) => {
            RESTORE_ORCHESTRATOR.get().unwrap().set_finished();
            Err(anyhow!("restore failed: {e}"))
        }
    };
}

pub async fn run_restore() -> anyhow::Result<()> {
    let config = CONFIG.get().unwrap();

    let orchestrator = RESTORE_ORCHESTRATOR.get().unwrap();
    orchestrator.set_started()?;

    // retrieve the snapshot id and contacted peers from the server
    let BackupRestoreInfo { snapshot_hash, peers } = requests::backup_restore().await?;

    log!("[restore] restoring from snapshot {}", hex::encode(snapshot_hash));
    // request all files from all peers
    for peer in peers {
        log!("[restore] requesting files from peer {}", hex::encode(peer));
        orchestrator.add_peer(peer).await;
        request_restore_from_peer(peer).await?;
    }

    // we will now wait for the requests to complete in the background
    loop {
        // if something fails, restore is no longer running
        if !orchestrator.is_running() {
            bail!("restore from some peers failed");
        }

        // if all peers have completed, we can stop waiting
        if orchestrator.all_peers_completed().await {
            break;
        }

        // waiting is fine for now
        sleep(std::time::Duration::from_secs(1)).await;
    }

    log!("[restore] now restoring files to the backup path...");
    // restore files to backup path
    dir_unpacker::unpack(
        config.get_restored_packfiles_folder()?,
        config
            .get_backup_path()
            .await?
            .ok_or(anyhow!("backup path not set"))?,
        snapshot_hash,
    )
    .await?;

    orchestrator.set_finished();
    log!("[restore] restore completed successfully!");
    Ok(())
}

async fn request_restore_from_peer(peer_id: ClientId) -> anyhow::Result<()> {
    let nonce = TRANSPORT_REQUESTS
        .get()
        .unwrap()
        .add_request(peer_id, RequestType::RestoreAll)
        .await?;
    requests::p2p_connection_begin(peer_id, nonce).await?;

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
