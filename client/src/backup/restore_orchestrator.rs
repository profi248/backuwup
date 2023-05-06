//! Implements coordination between various parts of the restore process.

use std::{collections::HashMap, sync::atomic::AtomicBool};

use anyhow::bail;
use shared::types::ClientId;
use tokio::sync::Mutex;

use crate::{
    backup::{BACKUP_ORCHESTRATOR, RESTORE_ORCHESTRATOR},
    UI,
};

/// Stores the state of the restore process, and provides options for
/// different moving parts to communicate with each other.
pub struct RestoreOrchestrator {
    restore_running: AtomicBool,
    peer_restore_status: Mutex<HashMap<ClientId, bool>>,
}

impl RestoreOrchestrator {
    /// Initialize the global state.
    pub async fn initialize_static() -> anyhow::Result<()> {
        match RESTORE_ORCHESTRATOR.get() {
            Some(orchestrator) => {
                if orchestrator.is_running() {
                    bail!("restore already running");
                }
                orchestrator.peer_restore_status.lock().await.clear();
            }
            None => {
                RESTORE_ORCHESTRATOR
                    .set(RestoreOrchestrator {
                        restore_running: AtomicBool::new(false),
                        peer_restore_status: Mutex::new(HashMap::new()),
                    })
                    .map_err(|_| anyhow::anyhow!("failed to initialize restore orchestrator"))?;
            }
        }

        Ok(())
    }

    /// Set the restore to started state.
    pub fn set_started(&self) -> anyhow::Result<()> {
        if let Some(orchestrator) = BACKUP_ORCHESTRATOR.get() {
            if orchestrator.is_backup_running() {
                bail!("backup already running, cannot start restore");
            }
        }

        UI.get().unwrap().send_restore_started();
        self.restore_running.store(true, std::sync::atomic::Ordering::Relaxed);

        Ok(())
    }

    /// Set the restore to finished state.
    pub fn set_finished(&self, success: bool, msg: impl Into<String>) {
        UI.get().unwrap().send_restore_finished(success, msg);
        self.restore_running
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }

    /// Check if the restore is running.
    pub fn is_running(&self) -> bool {
        self.restore_running.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Add a peer to the restore orchestrator.
    pub async fn add_peer(&self, client_id: ClientId) {
        let mut peer_restore_status = self.peer_restore_status.lock().await;
        peer_restore_status.insert(client_id, false);
    }

    /// Mark a peer as completed.
    pub async fn complete_peer(&self, client_id: ClientId) {
        let mut peer_restore_status = self.peer_restore_status.lock().await;
        peer_restore_status.insert(client_id, true);
    }

    /// Check if all peers have completed.
    pub async fn all_peers_completed(&self) -> bool {
        let peer_restore_status = self.peer_restore_status.lock().await;
        peer_restore_status.values().all(|v| *v)
    }
}
