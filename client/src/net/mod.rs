pub mod requests;

use std::convert::identity;

use futures_util::StreamExt;
use tokio::time;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, http::StatusCode, Error},
};

use crate::{identity, CONFIG, LOGGER};

const RETRY_INTERVAL: time::Duration = time::Duration::from_secs(5);

pub async fn init() {
    let logger = LOGGER.get().unwrap();
    let config = CONFIG.get().unwrap();

    // todo: make this a loop that tries to reconnect if the connection is lost
    let mut stream = loop {
        let mut request = "ws://localhost:8080/ws"
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
                logger.send("No auth token found, trying to log in...");
                if identity::login().await.is_ok() {
                    logger.send("Login successful!");
                    continue;
                } else {
                    logger.send("Login failed, will retry");
                    time::sleep(RETRY_INTERVAL).await;
                    continue;
                }
            }
            Err(e) => panic!("Failed to load auth token: {e}"),
        }

        match connect_async(request).await {
            Ok((stream, _)) => {
                logger.send("Connected to WebSocket server!");
                break stream;
            }
            Err(Error::Http(response)) => {
                if response.status() == StatusCode::UNAUTHORIZED {
                    logger.send("WebSocket unauthorized, trying to log in...");
                    config
                        .save_auth_token(None)
                        .await
                        .expect("Failed to wipe auth token");
                } else {
                    logger.send(format!(
                        "Unexpected response code when connecting to WebSocket server: {}",
                        response.status()
                    ));
                    time::sleep(RETRY_INTERVAL).await;
                }
            }
            Err(e) => {
                logger.send(format!("Error when connecting to WebSocket server: {e}"));
                time::sleep(RETRY_INTERVAL).await;
            }
        };
    };

    loop {
        let msg = stream.next().await.unwrap().expect("next message failed");
        println!("[ws] {msg}");
    }
}
