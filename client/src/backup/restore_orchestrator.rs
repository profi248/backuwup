use std::{collections::HashMap, sync::atomic::AtomicBool};

use anyhow::bail;
use shared::types::ClientId;
use tokio::sync::Mutex;

use crate::{
    backup::{BACKUP_ORCHESTRATOR, RESTORE_ORCHESTRATOR},
    UI,
};

pub struct RestoreOrchestrator {
    restore_running: AtomicBool,
    peer_restore_status: Mutex<HashMap<ClientId, bool>>,
}

impl RestoreOrchestrator {
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

    pub fn set_started(&self) -> anyhow::Result<()> {
        if let Some(orchestrator) = BACKUP_ORCHESTRATOR.get() {
            if orchestrator.is_backup_running() {
                bail!("backup already running, cannot start restore");
            }
        }

        UI.get().unwrap().set_restore_started();
        self.restore_running.store(true, std::sync::atomic::Ordering::Relaxed);

        Ok(())
    }

    pub fn set_finished(&self) {
        UI.get().unwrap().set_restore_finished();
        self.restore_running
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn is_running(&self) -> bool {
        self.restore_running.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub async fn add_peer(&self, client_id: ClientId) {
        let mut peer_restore_status = self.peer_restore_status.lock().await;
        peer_restore_status.insert(client_id, false);
    }

    pub async fn complete_peer(&self, client_id: ClientId) {
        let mut peer_restore_status = self.peer_restore_status.lock().await;
        peer_restore_status.insert(client_id, true);
    }

    pub async fn all_peers_completed(&self) -> bool {
        let peer_restore_status = self.peer_restore_status.lock().await;
        peer_restore_status.values().all(|v| *v)
    }
}
