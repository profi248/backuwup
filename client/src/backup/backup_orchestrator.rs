use std::{
    cmp::max,
    collections::HashMap,
    path::PathBuf,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::bail;
use shared::types::ClientId;
use tokio::sync::{oneshot, Mutex};

use crate::{backup::BACKUP_ORCHESTRATOR, log, net_p2p::transport::BackupTransportManager};

#[derive(Default, Debug)]
pub struct BackupOrchestrator {
    /// Active connections to other peers.
    pub(super) active_transport_sessions: Mutex<HashMap<ClientId, BackupTransportManager>>,
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
    /// The last time that backup storage was requested.
    storage_request_last_sent: AtomicU64,
    /// The last time that backup storage was matched.
    storage_request_last_matched: AtomicU64,
    /// Path to the backup destination (packfile folder).
    destination_path: PathBuf,
    /// Backup size estimate (calculated from input folder).
    size_estimate: AtomicU64,
}

impl BackupOrchestrator {
    pub async fn initialize_static(destination: &PathBuf) -> anyhow::Result<()> {
        match BACKUP_ORCHESTRATOR.get() {
            Some(orchestrator) => {
                if orchestrator.backup_running.load(Ordering::Acquire) {
                    bail!("backup already running");
                }

                // clear the state
                orchestrator.listeners.lock().await.clear();
                orchestrator.paused.store(false, Ordering::Release);
                orchestrator.packfile_bytes_written.store(0, Ordering::Relaxed);
                orchestrator.packfile_bytes_sent.store(0, Ordering::Relaxed);
                orchestrator.packing_completed.store(false, Ordering::Release);
                orchestrator.size_estimate.store(0, Ordering::Release);
            }
            None => {
                BACKUP_ORCHESTRATOR
                    .set(BackupOrchestrator {
                        destination_path: destination.clone(),
                        ..Default::default()
                    })
                    .unwrap();
            }
        }

        Ok(())
    }

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
            listener.send(()).ok();
        }
    }

    pub fn update_packfile_bytes_written(&self, bytes: u64) {
        self.packfile_bytes_written.store(bytes, Ordering::Relaxed);
    }

    pub fn get_packfile_bytes_written(&self) -> u64 {
        self.packfile_bytes_written.load(Ordering::Relaxed)
    }

    pub fn get_packfile_bytes_sent(&self) -> u64 {
        self.packfile_bytes_sent.load(Ordering::Relaxed)
    }

    pub fn available_packfile_bytes(&self) -> u64 {
        // this might not be entirely accurate, but it's available in real time,
        // we just need an estimate if enough data is available, and how much storage to request
        max(
            self.packfile_bytes_written.load(Ordering::Relaxed) as i64
                - self.packfile_bytes_sent.load(Ordering::Relaxed) as i64,
            0,
        ) as u64
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
        self.storage_request_last_sent
            .store(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(), Ordering::Release);
    }

    pub fn set_backup_started(&self) {
        self.backup_running.store(true, Ordering::Release);
    }

    pub fn set_backup_finished(&self) {
        self.backup_running.store(false, Ordering::Release);
    }

    pub fn is_backup_running(&self) -> bool {
        self.backup_running.load(Ordering::Acquire)
    }

    pub fn get_storage_request_last_matched(&self) -> u64 {
        self.storage_request_last_matched.load(Ordering::Acquire)
    }

    pub fn set_size_estimate(&self, size: u64) {
        self.size_estimate.store(size, Ordering::Release);
    }

    pub fn get_size_estimate(&self) -> u64 {
        self.size_estimate.load(Ordering::Acquire)
    }

    pub fn update_storage_request_last_matched(&self) {
        self.storage_request_last_matched
            .store(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(), Ordering::Release);
    }

    pub async fn is_peer_connected(&self, client_id: &ClientId) -> bool {
        self.active_transport_sessions.lock().await.contains_key(client_id)
    }

    pub async fn get_active_peers(&self) -> Vec<ClientId> {
        self.active_transport_sessions.lock().await.keys().copied().collect()
    }
}
