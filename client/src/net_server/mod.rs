//! Communicates with the server via WebSocket and HTTP requests.

pub mod requests;

use std::env;

use anyhow::anyhow;
use futures_util::StreamExt;
use shared::server_message_ws::ServerMessageWs;
use tokio::{net::TcpStream, time};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, http::StatusCode, Error, Message},
    MaybeTlsStream, WebSocketStream,
};

use crate::{backup::send, config::Config, identity, log, net_p2p::handle_connections, CONFIG, UI};

const RETRY_INTERVAL: time::Duration = time::Duration::from_secs(5);

/// Connects to the server via WebSocket and starts the message processing loop.
pub async fn connect_ws() {
    let logger = UI.get().unwrap();

    // server reconnection loop
    loop {
        let server_url = env::var("SERVER_ADDR").unwrap_or(crate::defaults::SERVER_ADDR.to_string());

        let protocol = match Config::use_tls() {
            true => "wss",
            false => "ws",
        };

        let endpoint = format!("{protocol}://{server_url}/ws");
        let mut stream = websocket_connect(endpoint).await;

        // message processing loop
        loop {
            match stream.next().await {
                None => {
                    logger.log("[net] WebSocket connection closed, will try to reconnect...");
                    break;
                }
                Some(Ok(msg)) => {
                    logger.log(format!("[net] message from server: {msg}"));
                    process_message(msg).await;
                }
                Some(Err(e)) => {
                    logger.log(format!("[net] Error: {e:?}, will try to reconnect..."));
                    break;
                }
            }
        }
    }
}

/// Processes a message received from the server.
async fn process_message(msg: Message) {
    // process the message in a separate task
    tokio::spawn(async {
        let msg: Option<ServerMessageWs> = match msg.into_text() {
            Ok(text) => match serde_json::from_str(&text) {
                Ok(msg) => Ok(msg),
                Err(e) => Err(anyhow!(e)),
            },
            Err(e) => Err(anyhow!(e)),
        }
        .map_err(|e| log!("[net] Received invalid message: {}", e))
        .ok();

        if let Some(msg) = msg {
            match msg {
                ServerMessageWs::Ping => Ok(()),
                ServerMessageWs::BackupMatched(request) => {
                    send::handle_storage_request_matched(request).await
                }
                // a peer wants to connect to us, listen on port
                ServerMessageWs::IncomingP2PConnection(request) => {
                    handle_connections::accept_and_listen(request).await
                }
                // we are are trying to connect to a peer and it just accepted, open a connection
                ServerMessageWs::FinalizeP2PConnection(request) => {
                    handle_connections::accept_and_connect(request).await
                }
            }
            .map_err(|e| log!("[net] Error processing incoming message: {}", e))
            .ok();
        }
    });
}

/// Connects to the server via WebSocket, logging in if necessary, and returns the stream.
async fn websocket_connect(endpoint: String) -> WebSocketStream<MaybeTlsStream<TcpStream>> {
    let logger = UI.get().unwrap();
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

                request.headers_mut().append("Authorization", token.parse().unwrap());
            }
            Ok(None) => {
                logger.log("[net] No auth token found, trying to log in...");
                if identity::login().await.is_ok() {
                    logger.log("Login successful!");
                    continue;
                } else {
                    logger.log("[net] Login failed, will retry");
                    time::sleep(RETRY_INTERVAL).await;
                    continue;
                }
            }
            Err(e) => panic!("Failed to load auth token: {e}"),
        }

        match connect_async(request).await {
            Ok((stream, _)) => {
                logger.log("[net] Connected to WebSocket server!");
                break stream;
            }
            Err(Error::Http(response)) => {
                if response.status() == StatusCode::UNAUTHORIZED {
                    logger.log("[net] WebSocket unauthorized, trying to log in...");

                    config.save_auth_token(None).await.expect("Failed to wipe auth token");
                } else {
                    logger.log(format!(
                        "[net] Unexpected response code when connecting to WebSocket server: {}",
                        response.status()
                    ));
                    time::sleep(RETRY_INTERVAL).await;
                }
            }
            Err(e) => {
                logger.log(format!("[net] Error when connecting to WebSocket server: {e}"));
                time::sleep(RETRY_INTERVAL).await;
            }
        };
    }
}
