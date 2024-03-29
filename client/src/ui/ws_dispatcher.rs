//! Dispatches incoming messages from the clients.

use std::path::PathBuf;

use anyhow::bail;
use futures_util::{stream::SplitStream, StreamExt};
use poem::web::websocket::{Message, WebSocketStream};
use serde::{Deserialize, Serialize};

use crate::{
    backup::{request_restore, run},
    ui::ws_status_message::Messenger,
    CONFIG, KEYS, UI,
};

#[derive(Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ClientMessage {
    Config(Config),
    StartBackup,
    GetConfig,
    StartRestore,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Config {
    pub client_id: String,
    pub path: Option<PathBuf>,
}

/// Dispatches messages from the WebSocket clients (the web UI) to the appropriate handlers.
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
            UI.get().unwrap().log(format!("error processing request: {e}"));
        };
    }
}

/// Processes a message received from the client and run the appropriate task.
async fn process_message(msg: &serde_json::Result<ClientMessage>) -> anyhow::Result<()> {
    match msg {
        Ok(ClientMessage::Config(conf)) => set_config(conf).await?,
        Ok(ClientMessage::StartBackup) => run().await?,
        Ok(ClientMessage::GetConfig) => send_config_message().await?,
        Ok(ClientMessage::StartRestore) => request_restore().await?,
        Err(e) => bail!("invalid message from client: {e:?}"),
    }

    Ok(())
}

/// Applies the received configuration.
async fn set_config(conf: &Config) -> anyhow::Result<()> {
    let config = CONFIG.get().unwrap();

    if let Some(path) = &conf.path {
        config.set_backup_path(path.clone()).await?;
    }

    Ok(())
}

/// Sends the current configuration to the client.
async fn send_config_message() -> anyhow::Result<()> {
    let config = CONFIG.get().unwrap();
    let client_id = Messenger::peer_id_display(&KEYS.get().unwrap().get_pubkey());

    UI.get()
        .unwrap()
        .send_config(Config { path: config.get_backup_path().await?, client_id });

    UI.get().unwrap().send_progress();

    Ok(())
}
