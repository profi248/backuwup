use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering::Relaxed;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use serde::Serialize;
use tokio::sync::broadcast::{Receiver, Sender};

#[derive(Debug)]
pub struct Logger {
    sender: Sender<LogItem>,
    current: AtomicU64,
    total: AtomicU64,
    last_sent: AtomicU64
}

#[derive(Clone, Serialize)]
pub enum LogItem {
    Message(String),
    Progress(Progress)
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Progress {
    current: u64,
    total: u64,
    file: String
}

impl Logger {
    pub fn new(sender: Sender<LogItem>) -> Self {
        Self { sender, current: Default::default(), total: Default::default(), last_sent: Default::default() }
    }

    pub fn send(&self, msg: impl Into<String> + Clone) {
        // ignore sending errors because they are not very meaningful
        self.sender.send(LogItem::Message(msg.clone().into())).ok();
        println!("[log] {}", msg.into());
    }


    pub fn progress_set_total(&self, total: u64) {
        self.total.store(total, Relaxed);
    }

    pub fn increment_progress(&self, file: impl Into<String>) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;

        // debounce progress updates, send at most once every 100ms
        if now - self.last_sent.load(Relaxed) >= 100 {
            self.sender.send(LogItem::Progress(Progress {
                current: self.current.fetch_add(1, Relaxed),
                total: self.total.load(Relaxed),
                file: file.into(),
            })).ok();

            self.last_sent.store(now, Relaxed);
        } else {
            self.current.fetch_add(1, Relaxed);
        }
    }

    pub fn subscribe(&self) -> Receiver<LogItem> {
        self.sender.subscribe()
    }
}
