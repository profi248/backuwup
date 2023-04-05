use std::{
    sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use tokio::sync::broadcast::{Receiver, Sender};

use crate::ui::ws_dispatcher::Config;

#[derive(Debug)]
pub struct Messenger {
    sender: Sender<StatusMessage>,
    current: AtomicU64,
    failed: AtomicU64,
    total: AtomicU64,
    last_sent: AtomicU64,
    running: AtomicBool,
}

#[derive(Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum StatusMessage {
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

impl Messenger {
    pub fn new(sender: Sender<StatusMessage>) -> Self {
        Self {
            sender,
            current: Default::default(),
            failed: Default::default(),
            total: Default::default(),
            last_sent: Default::default(),
            running: Default::default(),
        }
    }

    pub fn log(&self, msg: impl Into<String> + Clone) {
        // ignore sending errors because they are not very meaningful
        self.sender.send(StatusMessage::Message(msg.clone().into())).ok();
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
                .send(StatusMessage::Progress(Progress {
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

    pub fn progress_resend(&self) {
        if !self.running.load(Relaxed) {
            return;
        }

        self.sender
            .send(StatusMessage::Progress(Progress {
                current: self.current.load(Relaxed),
                total: self.total.load(Relaxed),
                failed: self.failed.load(Relaxed),
                file: "...".to_string(),
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
}

#[macro_export]
macro_rules! log {
    ($msg:expr, $($args:expr),*) => {
        $crate::UI.get().unwrap().log(format!($msg, $($args),*));
    };
}
