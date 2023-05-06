use std::{
    collections::HashSet,
    str::from_utf8,
    sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed},
    time::{SystemTime, UNIX_EPOCH},
};

use itertools::Itertools;
use serde::Serialize;
use shared::types::ClientId;
use tokio::sync::{
    broadcast::{Receiver, Sender},
    Mutex,
};

use crate::{backup::BACKUP_ORCHESTRATOR, ui::ws_dispatcher::Config};

/// Manage sending status messages to the WebSocket clients (the web user interface).
#[derive(Debug)]
pub struct Messenger {
    sender: Sender<StatusMessage>,
    current: AtomicU64,
    failed: AtomicU64,
    total: AtomicU64,
    last_sent: AtomicU64,
    last_sent_peers: AtomicU64,
    backup_running: AtomicBool,
    restore_running: AtomicBool,
    pack_running: AtomicBool,
    peers: Mutex<HashSet<ClientId>>,
}

#[derive(Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum StatusMessage {
    Message(String),
    Progress(Progress),
    Config(Config),
    BackupStarted,
    BackupFinished((bool, String)),
    RestoreStarted,
    RestoreFinished((bool, String)),
    Panic(String),
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Progress {
    current: u64,
    total: u64,
    failed: u64,
    file: String,
    size_estimate: u64,
    bytes_on_disk: u64,
    bytes_transmitted: u64,
    pack_running: bool,
    backup_running: bool,
    restore_running: bool,
    peers: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct BackupProgress {}

#[derive(Clone, Debug, Default, Serialize)]
pub struct RestoreProgress {}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Peer {
    id: String,
    transmitted: i64,
    negotiated: i64,
    connected: bool,
}

impl Messenger {
    /// Create a new `Messenger` instance.
    pub fn new(sender: Sender<StatusMessage>) -> Self {
        Self {
            sender,
            current: AtomicU64::default(),
            failed: AtomicU64::default(),
            total: AtomicU64::default(),
            last_sent: AtomicU64::default(),
            last_sent_peers: AtomicU64::default(),
            backup_running: AtomicBool::default(),
            restore_running: AtomicBool::default(),
            pack_running: AtomicBool::default(),
            peers: Mutex::default(),
        }
    }

    /// Let the actual WebSocket handler subscribe to the internal broadcast channel.
    pub fn subscribe(&self) -> Receiver<StatusMessage> {
        self.sender.subscribe()
    }

    /// Send a generic log message to the WebSocket clients.
    pub fn log(&self, msg: impl Into<String> + Clone) {
        // ignore sending errors because they are not very meaningful
        self.sender.send(StatusMessage::Message(msg.clone().into())).ok();
        println!("[log] {}", msg.into());
    }

    /// Send a panic (crash) message to the WebSocket clients.
    pub fn panic(&self, msg: impl Into<String> + Clone) {
        self.sender.send(StatusMessage::Panic(msg.into())).ok();
    }

    /// Add a peer to the list of peers that have been seen during the current backup.
    pub async fn progress_add_peer(&self, id: ClientId) {
        let mut peers = self.peers.lock().await;
        peers.insert(id);
    }

    /// Set the total number of files to be backed up.
    pub fn progress_set_total(&self, total: u64) {
        self.total.store(total, Relaxed);
    }

    /// Increment the number of files that have failed to up.
    pub fn progress_increment_failed(&self) {
        self.failed.fetch_add(1, Relaxed);
    }

    /// Send the current progress to the WebSocket clients, if there has been a certain delay.
    pub async fn progress_notify_increment(&self, file: impl Into<String>) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;

        let mut peers = None;

        // this is a slightly more expensive operation, get updates from orchestrator at most every 250 ms
        if now - self.last_sent_peers.load(Relaxed) >= 250 {
            peers = Some(self.peers.lock().await.iter().map(Self::peer_id_display).collect());

            self.last_sent_peers.store(now, Relaxed);
        }

        // debounce progress updates, send at most once every 100 ms
        if now - self.last_sent.load(Relaxed) >= 100 {
            let orchestrator = BACKUP_ORCHESTRATOR.get().unwrap();
            self.sender
                .send(StatusMessage::Progress(Progress {
                    current: self.current.fetch_add(1, Relaxed),
                    total: self.total.load(Relaxed),
                    failed: self.failed.load(Relaxed),
                    file: file.into(),
                    size_estimate: orchestrator.get_size_estimate(),
                    bytes_on_disk: orchestrator.get_packfile_bytes_written(),
                    bytes_transmitted: orchestrator.get_packfile_bytes_sent(),
                    pack_running: self.pack_running.load(Relaxed),
                    backup_running: self.backup_running.load(Relaxed),
                    restore_running: self.restore_running.load(Relaxed),
                    peers,
                }))
                .ok();

            self.last_sent.store(now, Relaxed);
        } else {
            self.current.fetch_add(1, Relaxed);
        }
    }

    /// Format peer id as a easily readable hex string (like an IPv6 address).
    fn peer_id_display(id: &ClientId) -> String {
        // we can convert from and to utf8 because it's all ascii
        hex::encode(id).as_bytes()[..]
            .chunks(2)
            .map(|c| from_utf8(c).unwrap())
            .join(":")
    }

    /// Send a progress update to the WebSocket clients.
    pub fn send_progress(&self) {
        if self.backup_running.load(Relaxed) {
            let orchestrator = BACKUP_ORCHESTRATOR.get().unwrap();

            self.sender
                .send(StatusMessage::Progress(Progress {
                    current: self.current.load(Relaxed),
                    total: self.total.load(Relaxed),
                    failed: self.failed.load(Relaxed),
                    file: "...".to_string(),
                    size_estimate: orchestrator.get_size_estimate(),
                    bytes_on_disk: orchestrator.get_packfile_bytes_written(),
                    bytes_transmitted: orchestrator.get_packfile_bytes_sent(),
                    pack_running: self.pack_running.load(Relaxed),
                    backup_running: self.backup_running.load(Relaxed),
                    restore_running: self.restore_running.load(Relaxed),
                    peers: None,
                }))
                .ok();
        } else if self.restore_running.load(Relaxed) {
            self.sender
                .send(StatusMessage::Progress(Progress {
                    current: self.current.load(Relaxed),
                    total: self.total.load(Relaxed),
                    failed: self.failed.load(Relaxed),
                    file: "...".to_string(),
                    size_estimate: 0,
                    bytes_on_disk: 0,
                    bytes_transmitted: 0,
                    pack_running: self.pack_running.load(Relaxed),
                    backup_running: self.backup_running.load(Relaxed),
                    restore_running: self.restore_running.load(Relaxed),
                    peers: None,
                }))
                .ok();
        }
    }

    /// Send a backup started message to the WebSocket clients.
    pub fn send_backup_started(&self) {
        self.backup_running.store(true, Relaxed);
        self.pack_running.store(true, Relaxed);
        self.sender.send(StatusMessage::BackupStarted).ok();
    }

    /// Send a backup finished message to the WebSocket clients.
    pub fn send_backup_finished(&self, success: bool, msg: impl Into<String>) {
        self.sender
            .send(StatusMessage::BackupFinished((success, msg.into())))
            .ok();
        self.total.store(0, Relaxed);
        self.current.store(0, Relaxed);
        self.failed.store(0, Relaxed);
        self.backup_running.store(false, Relaxed);
    }

    /// Send a config update to the WebSocket clients.
    pub fn send_config(&self, config: Config) {
        self.sender.send(StatusMessage::Config(config)).ok();
    }

    /// Set the pack running state.
    pub fn set_pack_running(&self, running: bool) {
        self.pack_running.store(running, Relaxed);
        self.send_progress();
    }

    /// Send a restore started message to the WebSocket clients.
    pub fn send_restore_started(&self) {
        self.restore_running.store(true, Relaxed);
        self.sender.send(StatusMessage::RestoreStarted).ok();
    }

    /// Send a restore finished message to the WebSocket clients.
    pub fn send_restore_finished(&self, success: bool, msg: impl Into<String>) {
        self.restore_running.store(false, Relaxed);
        self.sender
            .send(StatusMessage::RestoreFinished((success, msg.into())))
            .ok();
    }
}

#[macro_export]
macro_rules! log {
    ($msg:literal $(, $args:expr)*) => {
        { $crate::UI.get().unwrap().log(format!($msg, $($args),*)); }
    };
}
