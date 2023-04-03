use std::{
    cmp::min,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};

use anyhow::bail;
use fs_extra::dir::get_size;
use tokio::sync::{oneshot, Mutex, OnceCell};

use crate::{backup::filesystem::package, CONFIG, LOGGER};

pub mod filesystem;

pub static BACKUP_STATE: OnceCell<State> = OnceCell::const_new();

#[derive(Default, Debug)]
pub struct State {
    listeners: Mutex<Vec<oneshot::Sender<()>>>,
    paused: AtomicBool,
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
}

pub async fn run() -> anyhow::Result<()> {
    BACKUP_STATE.set(State::default())?;

    let config = CONFIG.get().unwrap();
    let backup_path = config.get_backup_path().await?;
    let destination = config.get_packfile_path().await?;

    if backup_path.is_none() {
        bail!("backup path not set");
    }

    let result = package::pack(backup_path.unwrap(), destination).await;

    match result {
        Ok(hash) => {
            LOGGER
                .get()
                .unwrap()
                .send(format!("backup done, snapshot hash: {}", hex::encode(hash)));
        }
        Err(e) => {
            LOGGER
                .get()
                .unwrap()
                .send_backup_finished(false, format!("Backup failed: {e:?}"));
        }
    };

    Ok(())
}

fn initial_storage_request_size(path: PathBuf) -> anyhow::Result<u64> {
    Ok(min(get_size(path)?, crate::defaults::MAX_PACKFILE_LOCAL_BUFFER_SIZE as u64))
}
