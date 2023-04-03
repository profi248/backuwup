use std::{
    cmp::min,
    path::PathBuf,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

use anyhow::bail;
use fs_extra::dir::get_size;
use shared::server_message_ws::{BackupMatched, FinalizeTransportRequest};
use tokio::sync::{oneshot, Mutex, OnceCell};

use crate::{
    backup::filesystem::package, net_server::requests, CONFIG, LOGGER, TRANSPORT_REQUESTS,
};

pub mod filesystem;

// todo add a lock to prevent multiple backups from running at the same time
pub static BACKUP_STATE: OnceCell<State> = OnceCell::const_new();

#[derive(Default, Debug)]
pub struct State {
    /// Channels to notify when the backup is resumed.
    listeners: Mutex<Vec<oneshot::Sender<()>>>,
    /// Whether the backup is currently paused, for example waiting for storage requests.
    paused: AtomicBool,
    /// The total number of bytes that have been written to all packfiles this session.
    packfile_bytes_written: AtomicU64,
    /// The total number of bytes of packfiles that have been sent to peers and deleted this session.
    packfile_bytes_sent: AtomicU64,
}

impl State {
    pub async fn pause(&self) -> oneshot::Receiver<()> {
        LOGGER.get().unwrap().send("backup is paused".to_string());
        self.paused.store(true, Ordering::Release);

        self.subscribe().await
    }

    pub fn should_continue(&self) -> bool {
        !self.paused.load(Ordering::Acquire)
    }

    pub async fn subscribe(&self) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();
        self.listeners.lock().await.push(tx);

        rx
    }

    pub async fn resume(&self) {
        LOGGER.get().unwrap().send("backup is resumed".to_string());
        self.paused.store(true, Ordering::Release);

        for listener in self.listeners.lock().await.drain(..) {
            listener.send(()).unwrap();
        }
    }

    pub fn update_packfile_bytes_written(&self, bytes: u64) {
        self.packfile_bytes_written.store(bytes, Ordering::Relaxed);
    }

    pub fn available_packfile_bytes(&self) -> u64 {
        // this might not be entirely accurate, but it's available in real time,
        // we just need an estimate if enough data is available, and how much storage to request
        self.packfile_bytes_written.load(Ordering::Relaxed)
            - self.packfile_bytes_sent.load(Ordering::Relaxed)
    }
}

pub async fn run() -> anyhow::Result<()> {
    BACKUP_STATE.set(State::default())?;

    let config = CONFIG.get().unwrap();
    let backup_path = config.get_backup_path().await?;
    let destination = config.get_packfile_path().await?;

    if backup_path.is_none() {
        bail!("backup path not set");
    }

    let result = tokio::spawn(package::pack(backup_path.unwrap(), destination));

    // wait for having enough data and send them in a loop (spawn it here)
    // probably notify the task when we got a request fulfilled

    match result.await {
        Ok(Ok(hash)) => {
            LOGGER
                .get()
                .unwrap()
                .send(format!("backup done, snapshot hash: {}", hex::encode(hash)));
        }
        Ok(Err(e)) => {
            LOGGER
                .get()
                .unwrap()
                .send_backup_finished(false, format!("Backup failed: {e:?}"));
        }
        Err(e) => {
            LOGGER
                .get()
                .unwrap()
                .send_backup_finished(false, format!("Unexpected backup error: {e:?}"));
        }
    };

    Ok(())
}

// todo properly handle errors
// todo store the storage granted to other peers in config
// todo maybe store to which peers we have sent a packfile
pub async fn handle_storage_request_matched(matched: BackupMatched) -> anyhow::Result<()> {
    let nonce = TRANSPORT_REQUESTS
        .get()
        .unwrap()
        .add_request(matched.destination_id)
        .await?;
    requests::backup_transport_begin(matched.destination_id, nonce)
        .await
        .unwrap();

    Ok(())
}

pub async fn handle_finalize_transport_request(
    request: FinalizeTransportRequest,
) -> anyhow::Result<()> {
    let mut transport = TRANSPORT_REQUESTS
        .get()
        .unwrap()
        .finalize_request(request.destination_client_id, request.destination_ip_address)
        .await
        .unwrap()
        .unwrap();

    // send the data

    transport.done().await.unwrap();

    Ok(())
}

pub async fn make_backup_storage_request(storage_required: u64) -> anyhow::Result<()> {
    requests::backup_storage_request(storage_required).await?;

    Ok(())
}

fn initial_storage_request_size(path: PathBuf) -> anyhow::Result<u64> {
    Ok(min(get_size(path)?, crate::defaults::MAX_PACKFILE_LOCAL_BUFFER_SIZE as u64))
}
