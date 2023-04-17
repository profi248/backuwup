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

#[derive(Debug)]
pub struct Messenger {
    sender: Sender<StatusMessage>,
    current: AtomicU64,
    failed: AtomicU64,
    total: AtomicU64,
    last_sent: AtomicU64,
    last_sent_peers: AtomicU64,
    running: AtomicBool,
    peers: Mutex<HashSet<ClientId>>,
}

// todo better support for restore and peer info
#[derive(Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum StatusMessage {
    Message(String),
    Progress(Progress),
    Config(Config),
    BackupStarted,
    BackupFinished((bool, String)),
    RestoreStarted,
    RestoreFinished,
    Panic(String),
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Progress {
    current: u64,
    total: u64,
    failed: u64,
    file: String,
    size_estimate: u64,
    bytes_written: u64,
    bytes_sent: u64,
    peers: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Peer {
    id: String,
    transmitted: i64,
    negotiated: i64,
    connected: bool,
}

impl Messenger {
    pub fn new(sender: Sender<StatusMessage>) -> Self {
        Self {
            sender,
            current: Default::default(),
            failed: Default::default(),
            total: Default::default(),
            last_sent: Default::default(),
            last_sent_peers: Default::default(),
            running: Default::default(),
            peers: Default::default(),
        }
    }

    pub fn log(&self, msg: impl Into<String> + Clone) {
        // ignore sending errors because they are not very meaningful
        self.sender.send(StatusMessage::Message(msg.clone().into())).ok();
        println!("[log] {}", msg.into());
    }

    pub fn panic(&self, msg: impl Into<String> + Clone) {
        self.sender.send(StatusMessage::Panic(msg.clone().into())).ok();
    }

    pub async fn progress_add_peer(&self, id: ClientId) {
        let mut peers = self.peers.lock().await;
        peers.insert(id);
    }

    pub fn progress_set_total(&self, total: u64) {
        self.total.store(total, Relaxed);
    }

    pub fn progress_increment_failed(&self) {
        self.failed.fetch_add(1, Relaxed);
    }

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
                    bytes_written: orchestrator.get_packfile_bytes_written(),
                    bytes_sent: orchestrator.get_packfile_bytes_sent(),
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

    pub fn progress_resend(&self) {
        if !self.running.load(Relaxed) {
            return;
        }

        let orchestrator = BACKUP_ORCHESTRATOR.get().unwrap();

        self.sender
            .send(StatusMessage::Progress(Progress {
                current: self.current.load(Relaxed),
                total: self.total.load(Relaxed),
                failed: self.failed.load(Relaxed),
                file: "...".to_string(),
                size_estimate: orchestrator.get_size_estimate(),
                bytes_written: orchestrator.get_packfile_bytes_written(),
                bytes_sent: orchestrator.get_packfile_bytes_sent(),
                peers: None,
            }))
            .ok();
    }

    pub fn send_backup_started(&self) {
        self.running.store(true, Relaxed);
        self.sender.send(StatusMessage::BackupStarted).ok();
    }

    pub fn send_backup_finished(&self, success: bool, msg: impl Into<String>) {
        self.sender
            .send(StatusMessage::BackupFinished((success, msg.into())))
            .ok();
        self.total.store(0, Relaxed);
        self.current.store(0, Relaxed);
        self.failed.store(0, Relaxed);
        self.running.store(false, Relaxed);
    }

    pub fn send_config(&self, config: Config) {
        self.sender.send(StatusMessage::Config(config)).ok();
    }

    pub fn subscribe(&self) -> Receiver<StatusMessage> {
        self.sender.subscribe()
    }

    pub fn set_restore_started(&self) {
        self.sender.send(StatusMessage::RestoreStarted).ok();
    }

    pub fn set_restore_finished(&self) {
        self.sender.send(StatusMessage::RestoreFinished).ok();
    }
}

#[macro_export]
macro_rules! log {
    ($msg:literal $(, $args:expr)*) => {
        { $crate::UI.get().unwrap().log(format!($msg, $($args),*)); }
    };
}
