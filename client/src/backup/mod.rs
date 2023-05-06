//! Contains all the logic for creating and restoring backups, including sending and
//! receiving files over the network and packing/unpacking them.

use std::path::PathBuf;

use anyhow::{anyhow, bail};
use backup_orchestrator::BackupOrchestrator;
use cast::From;
use fs_extra::dir::get_size;
use futures_util::{try_join, FutureExt};
use human_bytes::human_bytes;
use shared::{p2p_message::RequestType, server_message::BackupRestoreInfo, types::ClientId};
use tokio::{sync::OnceCell, time::sleep};

use crate::{
    backup::{
        filesystem::{dir_packer, dir_unpacker},
        restore_orchestrator::RestoreOrchestrator,
    },
    log,
    net_server::requests,
    CONFIG, P2P_CONN_REQUESTS, UI,
};

pub mod backup_orchestrator;
pub mod filesystem;
pub mod restore_orchestrator;
pub mod restore_send;
pub mod send;

/// A global state of the backup process, used for coordinating all components.
pub static BACKUP_ORCHESTRATOR: OnceCell<BackupOrchestrator> = OnceCell::const_new();
/// A global state of the restore process, used for coordinating all components.
pub static RESTORE_ORCHESTRATOR: OnceCell<RestoreOrchestrator> = OnceCell::const_new();

/// Start the entire backup process, spawning tasks for packing and sending files.
pub async fn run() -> anyhow::Result<()> {
    let config = CONFIG.get().unwrap();
    let destination = config.get_packfile_path()?;

    BackupOrchestrator::initialize_static(&destination).await?;

    if let Some(restore_orchestrator) = RESTORE_ORCHESTRATOR.get() {
        if restore_orchestrator.is_running() {
            bail!("cannot backup, restore already in progress");
        }
    }

    let backup_path = config.get_backup_path().await?;
    if backup_path.is_none() {
        UI.get()
            .unwrap()
            .send_backup_finished(false, "Backup failed: backup path not set");

        bail!("backup path not set");
    }

    BACKUP_ORCHESTRATOR.get().unwrap().set_backup_started();

    // create a size estimate to use with storage requests
    estimate_size(backup_path.as_ref().unwrap()).await;

    // start tasks for filesystem walking and for sending to peers
    let pack_result = tokio::spawn(dir_packer::pack(backup_path.clone().unwrap(), destination.clone()));
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

            config
                .log_backup(
                    i64::cast(BACKUP_ORCHESTRATOR.get().unwrap().get_size_estimate()).expect("bad size"),
                    backup_path.as_ref().unwrap(),
                )
                .await?;

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

/// Initialize the restore process, requesting files from peers and unpacking them.
pub async fn request_restore() -> anyhow::Result<()> {
    RestoreOrchestrator::initialize_static().await?;
    RESTORE_ORCHESTRATOR.get().unwrap().set_started()?;
    return match run_restore().await {
        Ok(_) => Ok(()),
        Err(e) => {
            RESTORE_ORCHESTRATOR.get().unwrap().set_finished(false, e.to_string());
            Err(anyhow!("restore failed: {e}"))
        }
    };
}

/// Run the actual restore procedure.
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

    UI.get().unwrap().set_pack_running(true);

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

    let packfile_size = get_size(config.get_restored_packfiles_folder()?)?;

    orchestrator.set_finished(
        true,
        format!(
            "Restore completed successfully!\nReceived and unpacked {} worth of data.",
            human_bytes(packfile_size as f64)
        ),
    );

    log!("[restore] restore completed successfully!");
    Ok(())
}

/// Request all files from a peer for restoration.
async fn request_restore_from_peer(peer_id: ClientId) -> anyhow::Result<()> {
    let nonce = P2P_CONN_REQUESTS
        .get()
        .unwrap()
        .add_request(peer_id, RequestType::RestoreAll)
        .await?;
    requests::p2p_connection_begin(peer_id, nonce).await?;

    Ok(())
}

/// Estimate the size of the data currently being backed up.
async fn estimate_size(backup_path: &PathBuf) {
    match get_size(backup_path) {
        Ok(size) => {
            // use a constant to estimate compression of a typical dataset,
            // we could do something smarter based on file types, but this is good enough for now
            let new_size = (size as f64 * 0.9) as u64;

            // try to get the difference from the previous backup, if the previous backup doesn't
            // exist or the path is different, we will just use the new size
            let difference = CONFIG
                .get()
                .unwrap()
                .get_backup_size_difference(i64::cast(new_size).expect("bad size"), backup_path)
                .await;

            let estimate = match difference {
                Ok(Some(0)) => 0,                                          // no difference
                Ok(Some(..=0)) => new_size,                                // new backup is smaller, we can't estimate
                Ok(Some(size)) => u64::cast(size).expect("bad size"), // new backup is bigger, use the difference
                _ => new_size,                                             // no valid previous backup, use the new size
            };

            BACKUP_ORCHESTRATOR.get().unwrap().set_size_estimate(estimate);
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
