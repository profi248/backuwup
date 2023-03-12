pub mod requests;

use futures_util::StreamExt;
use tokio::{net::TcpStream, time};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, http::StatusCode, Error},
    MaybeTlsStream, WebSocketStream,
};

use crate::{identity, CONFIG, LOGGER};

const RETRY_INTERVAL: time::Duration = time::Duration::from_secs(5);

pub async fn listen() {
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
                }
                Some(Err(e)) => {
                    logger.send(format!("[net] Error: {e:?}, will try to reconnect..."));
                    break;
                }
            }
        }
    }
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
