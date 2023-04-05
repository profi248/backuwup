pub mod requests;

use anyhow::anyhow;
use futures_util::StreamExt;
use shared::server_message_ws::ServerMessageWs;
use tokio::{net::TcpStream, time};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, http::StatusCode, Error, Message},
    MaybeTlsStream, WebSocketStream,
};

use crate::{identity, packfile_receiver, CONFIG, LOGGER, TRANSPORT_REQUESTS};
use crate::backup::send::{handle_finalize_transport_request, handle_storage_request_matched};

const RETRY_INTERVAL: time::Duration = time::Duration::from_secs(5);

pub async fn connect_ws() {
    let logger = LOGGER.get().unwrap();

    // server reconnection loop
    loop {
        let endpoint = format!("ws://{}/ws", crate::defaults::SERVER_URL);
        let mut stream = websocket_connect(endpoint).await;

        // message processing loop
        loop {
            match stream.next().await {
                None => {
                    logger.send("[net] WebSocket connection closed, will try to reconnect...");
                    break;
                }
                Some(Ok(msg)) => {
                    logger.send(format!("[net] message from server: {msg}"));

                    process_message(msg).await;
                }
                Some(Err(e)) => {
                    logger.send(format!("[net] Error: {e:?}, will try to reconnect..."));
                    break;
                }
            }
        }
    }
}

async fn process_message(msg: Message) {
    // process the message in a separate task
    tokio::spawn(async {
        let msg: Option<ServerMessageWs> = match msg.into_text() {
            Ok(text) => match serde_json::from_str(&text) {
                Ok(msg) => Ok(msg),
                Err(e) => Err(anyhow!(e))
            },
            Err(e) => Err(anyhow!(e))
        }.map_err(|e| LOGGER.get().unwrap().send(format!("[net] Received invalid message: {e}"))).ok();

        if msg.is_none() { return }
        let msg = msg.unwrap();

        match msg {
            ServerMessageWs::Ping => Ok(()),
            ServerMessageWs::BackupMatched(request) => handle_storage_request_matched(request).await,
            ServerMessageWs::IncomingTransportRequest(request) => packfile_receiver::receive_request(request).await,
            ServerMessageWs::FinalizeTransportRequest(request) => handle_finalize_transport_request(request).await,
            ServerMessageWs::StorageChallengeRequest(_) => Ok(()),
        }.map_err(|e| LOGGER.get().unwrap().send(format!("[net] Error processing incoming message: {e}"))).ok();

    });
}

async fn websocket_connect(endpoint: String) -> WebSocketStream<MaybeTlsStream<TcpStream>> {
    let logger = LOGGER.get().unwrap();
    let config = CONFIG.get().unwrap();

    // ideally use a smarter backoff strategy
    loop {
        let mut request = endpoint
            .clone()
            .into_client_request()
            .expect("Failed to create request");

        match config.load_auth_token().await {
            Ok(Some(token)) => {
                let token = hex::encode(token);

                request
                    .headers_mut()
                    .append("Authorization", token.parse().unwrap());
            }
            Ok(None) => {
                logger.send("[net] No auth token found, trying to log in...");
                if identity::login().await.is_ok() {
                    logger.send("Login successful!");
                    continue;
                } else {
                    logger.send("[net] Login failed, will retry");
                    time::sleep(RETRY_INTERVAL).await;
                    continue;
                }
            }
            Err(e) => panic!("Failed to load auth token: {e}"),
        }

        match connect_async(request).await {
            Ok((stream, _)) => {
                logger.send("[net] Connected to WebSocket server!");
                break stream;
            }
            Err(Error::Http(response)) => {
                if response.status() == StatusCode::UNAUTHORIZED {
                    logger.send("[net] WebSocket unauthorized, trying to log in...");

                    config
                        .save_auth_token(None)
                        .await
                        .expect("Failed to wipe auth token");
                } else {
                    logger.send(format!(
                        "[net] Unexpected response code when connecting to WebSocket server: {}",
                        response.status()
                    ));
                    time::sleep(RETRY_INTERVAL).await;
                }
            }
            Err(e) => {
                logger.send(format!("[net] Error when connecting to WebSocket server: {e}"));
                time::sleep(RETRY_INTERVAL).await;
            }
        };
    }
}
