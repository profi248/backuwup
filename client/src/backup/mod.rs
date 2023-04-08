use std::{
    collections::HashMap,
    path::PathBuf,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use std::cmp::max;

use anyhow::bail;
use futures_util::{try_join, FutureExt};
use shared::types::ClientId;
use tokio::sync::{oneshot, Mutex, OnceCell};

use crate::{backup::filesystem::dir_packer, net_p2p::transport::BackupTransportManager, CONFIG, UI, log};

pub mod filesystem;
pub mod send;

pub static BACKUP_ORCHESTRATOR: OnceCell<Orchestrator> = OnceCell::const_new();

#[derive(Default, Debug)]
pub struct Orchestrator {
    /// Channels to notify when the backup is resumed.
    listeners: Mutex<Vec<oneshot::Sender<()>>>,
    /// Whether the backup is currently paused, for example waiting for storage requests.
    paused: AtomicBool,
    /// The total number of bytes that have been written to all packfiles this session.
    packfile_bytes_written: AtomicU64,
    /// The total number of bytes of packfiles that have been sent to peers and deleted this session.
    packfile_bytes_sent: AtomicU64,
    /// Indicates whether a backup is already in progress to prevent multiple backups from running at the same time.
    backup_running: AtomicBool,
    /// Indicates whether all the local files have been packed.
    packing_completed: AtomicBool,
    /// Active connections to other peers.
    active_transport_sessions: Mutex<HashMap<ClientId, BackupTransportManager>>,
    /// The last time that backup storage was requested.
    storage_request_last_sent: AtomicU64,
    /// The last time that backup storage was matched.
    storage_request_last_matched: AtomicU64,
    /// Path to the backup destination (packfile folder).
    destination_path: PathBuf,
}

impl Orchestrator {
    pub async fn pause(&self) -> oneshot::Receiver<()> {
        log!("[orchestrator] backup is paused");
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
        log!("[orchestrator] backup is resumed");
        self.paused.store(false, Ordering::Release);

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
        max(self.packfile_bytes_written.load(Ordering::Relaxed) as i64
            - self.packfile_bytes_sent.load(Ordering::Relaxed) as i64, 0) as u64
    }

    pub fn increment_packfile_bytes_sent(&self, bytes: u64) {
        self.packfile_bytes_sent.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn packfile_bytes_written(&self) -> u64 {
        self.packfile_bytes_written.load(Ordering::Relaxed)
    }

    pub fn is_packing_completed(&self) -> bool {
        self.packing_completed.load(Ordering::Acquire)
    }

    pub fn set_packing_completed(&self) {
        self.packing_completed.store(true, Ordering::Release);
    }

    pub fn get_storage_request_last_sent(&self) -> u64 {
        self.storage_request_last_sent.load(Ordering::Acquire)
    }

    pub fn update_storage_request_last_sent(&self) {
        self.storage_request_last_sent.store(
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            Ordering::Release,
        );
    }

    pub fn get_storage_request_last_matched(&self) -> u64 {
        self.storage_request_last_matched.load(Ordering::Acquire)
    }

    pub fn update_storage_request_last_matched(&self) {
        self.storage_request_last_matched.store(
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            Ordering::Release,
        );
    }
}

pub async fn run() -> anyhow::Result<()> {
    let config = CONFIG.get().unwrap();
    let destination = config.get_packfile_path().await?;

    match BACKUP_ORCHESTRATOR.get() {
        Some(orchestrator) => {
            if orchestrator.backup_running.load(Ordering::Relaxed) {
                bail!("backup already running");
            }

            // clear the state
            orchestrator.listeners.lock().await.clear();
            orchestrator.paused.store(false, Ordering::Relaxed);
            orchestrator.packfile_bytes_written.store(0, Ordering::Relaxed);
            orchestrator.packfile_bytes_sent.store(0, Ordering::Relaxed);
        }
        None => {
            BACKUP_ORCHESTRATOR
                .set(Orchestrator {
                    backup_running: AtomicBool::new(true),
                    destination_path: destination.clone(),
                    ..Default::default()
                })
                .unwrap();
        }
    }

    let backup_path = config.get_backup_path().await?;

    if backup_path.is_none() {
        bail!("backup path not set")
    }

    BACKUP_ORCHESTRATOR
        .get()
        .unwrap()
        .backup_running
        .store(true, Ordering::Relaxed);

    let pack_result = tokio::spawn(dir_packer::pack(backup_path.unwrap(), destination.clone()));
    let transport_result = tokio::spawn(send::send(destination));

    // unpack the inner result so it stops whenever one of the tasks returns an error
    let result = try_join!(pack_result.map(|r| r.unwrap()), transport_result.map(|r| r.unwrap()));

    // todo if packer fails, send might keep running
    BACKUP_ORCHESTRATOR
        .get()
        .unwrap()
        .backup_running
        .store(false, Ordering::Relaxed);

    match result {
        Ok((hash, _)) => {
            UI.get().unwrap().send_backup_finished(
                true,
                "Backup completed successfully!"
            );

            UI.get().unwrap().log(format!(
                "Backup completed successfully! Snapshot hash: {}",
                hex::encode(hash)
            ));
        }
        Err(e) => {
            UI.get()
                .unwrap()
                .send_backup_finished(false, format!("Backup failed: {e:?}"));
        }
    };

    Ok(())
}
