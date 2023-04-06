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

use crate::{identity, packfile_receiver, CONFIG, UI};
use crate::backup::send::{handle_finalize_transport_request, handle_storage_request_matched};

const RETRY_INTERVAL: time::Duration = time::Duration::from_secs(5);

pub async fn connect_ws() {
    let logger = UI.get().unwrap();

    // server reconnection loop
    loop {
        let endpoint = format!("ws://{}/ws", crate::defaults::SERVER_URL);
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

async fn process_message(msg: Message) {
    // process the message in a separate task
    tokio::spawn(async {
        let msg: Option<ServerMessageWs> = match msg.into_text() {
            Ok(text) => match serde_json::from_str(&text) {
                Ok(msg) => Ok(msg),
                Err(e) => Err(anyhow!(e))
            },
            Err(e) => Err(anyhow!(e))
        }.map_err(|e| UI.get().unwrap().log(format!("[net] Received invalid message: {e}"))).ok();

        if msg.is_none() { return }
        let msg = msg.unwrap();

        match msg {
            ServerMessageWs::Ping => Ok(()),
            ServerMessageWs::BackupMatched(request) => handle_storage_request_matched(request).await,
            ServerMessageWs::IncomingTransportRequest(request) => packfile_receiver::receive_request(request).await,
            ServerMessageWs::FinalizeTransportRequest(request) => handle_finalize_transport_request(request).await,
            // ServerMessageWs::FinalizeTransportRequest(request) => {
            //     // todo temp test
            //     let mut mgr = TRANSPORT_REQUESTS
            //         .get()
            //         .unwrap()
            //         .finalize_request(request.destination_client_id, request.destination_ip_address)
            //         .await
            //         .unwrap()
            //         .unwrap();
            //
            //     for i in 0..10 {
            //         mgr.send_data(vec![i], [i; 12]).await.unwrap();
            //         time::sleep(time::Duration::from_secs(5)).await;
            //     }
            //
            //     mgr.done().await;
            //
            //     Ok(())
            // },
            ServerMessageWs::StorageChallengeRequest(_) => Ok(()),
        }.map_err(|e| UI.get().unwrap().log(format!("[net] Error processing incoming message: {e}"))).ok();

    });
}

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

                request
                    .headers_mut()
                    .append("Authorization", token.parse().unwrap());
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

                    config
                        .save_auth_token(None)
                        .await
                        .expect("Failed to wipe auth token");
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
