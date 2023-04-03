use std::{
    sync::atomic::{AtomicU64, Ordering::Relaxed},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use tokio::sync::broadcast::{Receiver, Sender};

use crate::ui::ws_dispatcher::Config;

#[derive(Debug)]
pub struct Logger {
    sender: Sender<LogItem>,
    current: AtomicU64,
    failed: AtomicU64,
    total: AtomicU64,
    last_sent: AtomicU64,
}

#[derive(Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum LogItem {
    Message(String),
    Progress(Progress),
    Config(Config),
    BackupStarted,
    BackupFinished((bool, String)),
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Progress {
    current: u64,
    total: u64,
    failed: u64,
    file: String,
}

impl Logger {
    pub fn new(sender: Sender<LogItem>) -> Self {
        Self {
            sender,
            current: Default::default(),
            failed: Default::default(),
            total: Default::default(),
            last_sent: Default::default(),
        }
    }

    pub fn send(&self, msg: impl Into<String> + Clone) {
        // ignore sending errors because they are not very meaningful
        self.sender.send(LogItem::Message(msg.clone().into())).ok();
        println!("[log] {}", msg.into());
    }

    pub fn progress_set_total(&self, total: u64) {
        self.total.store(total, Relaxed);
    }

    pub fn progress_increment_failed(&self) {
        self.failed.fetch_add(1, Relaxed);
    }

    pub fn progress_notify_increment(&self, file: impl Into<String>) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;

        // debounce progress updates, send at most once every 100ms
        if now - self.last_sent.load(Relaxed) >= 100 {
            self.sender
                .send(LogItem::Progress(Progress {
                    current: self.current.fetch_add(1, Relaxed),
                    total: self.total.load(Relaxed),
                    failed: self.failed.load(Relaxed),
                    file: file.into(),
                }))
                .ok();

            self.last_sent.store(now, Relaxed);
        } else {
            self.current.fetch_add(1, Relaxed);
        }
    }

    pub fn send_backup_started(&self) {
        self.sender.send(LogItem::BackupStarted).ok();
    }

    pub fn send_backup_finished(&self, success: bool, msg: impl Into<String>) {
        self.sender
            .send(LogItem::BackupFinished((success, msg.into())))
            .ok();
        self.total.store(0, Relaxed);
        self.current.store(0, Relaxed);
        self.failed.store(0, Relaxed);
    }

    pub fn send_config(&self, config: Config) {
        self.sender.send(LogItem::Config(config)).ok();
    }

    pub fn subscribe(&self) -> Receiver<LogItem> {
        self.sender.subscribe()
    }
}
