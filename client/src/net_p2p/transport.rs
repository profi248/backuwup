use std::{thread::sleep, time::Duration};

use anyhow::bail;
use futures_util::SinkExt;
use sha2::{Digest, Sha256};
use shared::{
    p2p_message::{EncapsulatedPackfile, PackfileBody, MAX_ENCAPSULATED_BACKUP_CHUNK_SIZE},
    types::TransportSessionNonce,
};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async, connect_async_with_config,
    tungstenite::{handshake::client::Response, protocol::WebSocketConfig, Error, Message},
    MaybeTlsStream, WebSocketStream,
};

use crate::{net_p2p::get_ws_config, KEYS, LOGGER};

pub struct BackupTransportManager {
    socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
    msg_counter: u64,
    session_nonce: TransportSessionNonce,
}

impl BackupTransportManager {
    pub async fn new(addr: String, session_nonce: TransportSessionNonce) -> anyhow::Result<Self> {
        let url = format!("ws://{addr}");

        // wait for the other party
        tokio::time::sleep(Duration::from_secs(1)).await;

        let mut attempts = 3;
        LOGGER
            .get()
            .unwrap()
            .send(format!("[p2p] trying to connect to peer at {url}"));
        let socket = loop {
            // retry in case the socket takes a while to open
            match connect_async_with_config(&url, Some(get_ws_config())).await {
                Ok((socket, _)) => break socket,
                Err(Error::Io(e)) => {
                    if attempts > 0 {
                        LOGGER.get().unwrap().send(format!(
                            "[p2p] failed to connect to peer: {e}, will try {attempts} more times"
                        ));
                        tokio::time::sleep(Duration::from_secs(3)).await;
                        attempts -= 1;
                        continue;
                    } else {
                        bail!("Unable to connect to peer after 3 retries: {e}")
                    }
                }
                Err(e) => bail!(e),
            };
        };

        LOGGER.get().unwrap().send("[p2p] connected successfully");
        Ok(BackupTransportManager {
            socket,
            msg_counter: 0,
            session_nonce,
        })
    }

    pub async fn send_data(&mut self, data: Vec<u8>) -> anyhow::Result<()> {
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let hash = hasher.finalize();

        let body = PackfileBody {
            sequence_number: self.msg_counter,
            session_nonce: self.session_nonce,
            hash: hash.into(),
            data,
        };

        self.msg_counter += 1;

        let body = bincode::serialize(&body)?;
        let signature = KEYS.get().unwrap().sign(&body).to_vec();

        let encapsulated = EncapsulatedPackfile { body, signature };

        let msg = bincode::serialize(&encapsulated)?;
        self.socket.send(Message::Binary(msg)).await?;

        Ok(())
    }

    pub async fn done(mut self) -> anyhow::Result<()> {
        self.socket.close(None).await?;
        Ok(())
    }
}
