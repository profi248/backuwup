use std::path::PathBuf;

use anyhow::bail;
use futures_util::{stream::SplitStream, StreamExt};
use poem::web::websocket::{Message, WebSocketStream};
use serde::{Deserialize, Serialize};

use crate::{backup::filesystem::package, CONFIG, LOGGER};

#[derive(Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ClientMessage {
    Config(Config),
    StartBackup,
    GetConfig,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Config {
    pub path: Option<PathBuf>,
}

pub async fn dispatch_commands(mut ws_recv: SplitStream<WebSocketStream>) {
    loop {
        let msg = match ws_recv.next().await {
            // for when the socket has closed
            None => break,
            Some(Ok(msg)) => msg,
            Some(Err(e)) => {
                println!("error: {e:?}");
                break;
            }
        };

        let msg: serde_json::Result<ClientMessage> = match msg {
            Message::Text(s) => serde_json::from_str(&s),
            _ => continue,
        };

        if let Err(e) = process_message(&msg).await {
            LOGGER
                .get()
                .unwrap()
                .send(format!("error processing request: {e}"));
        };
    }
}

async fn process_message(msg: &serde_json::Result<ClientMessage>) -> anyhow::Result<()> {
    match msg {
        Ok(ClientMessage::Config(conf)) => set_config(conf).await?,
        Ok(ClientMessage::StartBackup) => start_backup_from_config().await?,
        Ok(ClientMessage::GetConfig) => send_config_message().await?,
        Err(e) => bail!("invalid message from client: {e:?}"),
    }

    Ok(())
}

async fn start_backup_from_config() -> anyhow::Result<()> {
    let config = CONFIG.get().unwrap();
    let backup_path = config.get_backup_path().await?;
    let destination = config.get_packfile_path().await?;

    if let Some(path) = backup_path {
        tokio::spawn(async move {
            let result = package::pack(path, destination).await;

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
            }
        });
    } else {
        bail!("backup path is not set");
    }

    Ok(())
}

async fn set_config(conf: &Config) -> anyhow::Result<()> {
    let config = CONFIG.get().unwrap();

    if let Some(path) = &conf.path {
        config.set_backup_path(path.clone()).await?;
    }

    Ok(())
}

async fn send_config_message() -> anyhow::Result<()> {
    let config = CONFIG.get().unwrap();

    LOGGER.get().unwrap().send_config(Config {
        path: config.get_backup_path().await?,
    });

    Ok(())
}
