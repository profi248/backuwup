use std::time::Duration;

use anyhow::bail;
use ed25519_dalek::{PublicKey, Signature};
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use shared::{
    p2p_message::{AckBody, EncapsulatedPackfile, EncapsulatedPackfileAck, Header, PackfileBody},
    types::{ClientId, PackfileId, TransportSessionNonce},
};
use tokio::{net::TcpStream, sync::broadcast, time::timeout};
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::{Error, Message},
    MaybeTlsStream, WebSocketStream,
};

use crate::{
    defaults::{PACKFILE_ACK_TIMEOUT, PACKFILE_SEND_TIMEOUT},
    log,
    net_p2p::get_ws_config,
    KEYS,
};

#[derive(Debug)]
pub struct BackupTransportManager {
    tx: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
    msg_counter: u64,
    session_nonce: TransportSessionNonce,
    ack_notifier: broadcast::Sender<u64>,
    ack_handle: tokio::task::JoinHandle<()>,
}

impl BackupTransportManager {
    pub async fn new(
        addr: String,
        session_nonce: TransportSessionNonce,
        client_id: ClientId,
    ) -> anyhow::Result<Self> {
        let url = format!("ws://{addr}");

        // wait for the other party
        tokio::time::sleep(Duration::from_secs(1)).await;

        log!("[p2p] trying to connect to peer at {}", url);
        let socket = Self::sock_conn(&url).await?;
        log!("[p2p] connected successfully");

        let (tx, rx) = socket.split();
        let (ack_sender, _) = broadcast::channel(100);

        let mgr = BackupTransportManager {
            tx,
            msg_counter: 0,
            session_nonce,
            ack_notifier: ack_sender.clone(),
            ack_handle: tokio::spawn(Self::process_acks(rx, ack_sender, session_nonce, client_id)),
        };

        Ok(mgr)
    }

    async fn sock_conn(url: &String) -> anyhow::Result<WebSocketStream<MaybeTlsStream<TcpStream>>> {
        let mut attempts = 3;

        Ok(loop {
            // retry in case the socket takes a while to open
            match connect_async_with_config(url, Some(get_ws_config())).await {
                Ok((socket, _)) => break socket,
                Err(Error::Io(e)) => {
                    if attempts > 0 {
                        log!(
                            "[p2p] failed to connect to peer: {}, will try {} more times",
                            e,
                            attempts
                        );
                        tokio::time::sleep(Duration::from_secs(3)).await;
                        attempts -= 1;
                        continue;
                    } else {
                        bail!("Unable to connect to peer after 3 retries: {e}")
                    }
                }
                Err(e) => bail!(e),
            };
        })
    }

    pub fn parse_incoming_ack(
        data: &[u8],
        nonce: TransportSessionNonce,
        sender_pubkey: ClientId,
        messages_received: &mut u64,
    ) -> anyhow::Result<u64> {
        let encapsulated: EncapsulatedPackfileAck = bincode::deserialize(&data)?;

        // verify the signature
        let source_pubkey = PublicKey::from_bytes(&sender_pubkey)?;
        let signature = Signature::from_bytes(&encapsulated.signature)?;
        source_pubkey.verify_strict(&encapsulated.body, &signature)?;

        let body: AckBody = bincode::deserialize(&encapsulated.body).unwrap();

        // verify the replay protection header
        if body.header.session_nonce != nonce || body.header.sequence_number != *messages_received {
            bail!("invalid replay protection header in ack");
        }

        *messages_received += 1;

        Ok(body.acknowledged_sequence_number)
    }

    pub async fn process_acks(
        mut stream: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
        notifier: broadcast::Sender<u64>,
        nonce: TransportSessionNonce,
        sender_pubkey: ClientId,
    ) {
        let mut messages_received: u64 = 0;
        while let Some(msg) = stream.next().await {
            match msg {
                Ok(Message::Binary(data)) => match Self::parse_incoming_ack(
                    &data,
                    nonce,
                    sender_pubkey,
                    &mut messages_received,
                ) {
                    Ok(message_id) => {
                        notifier.send(message_id).ok();
                    }
                    Err(e) => {
                        log!("[p2p] error while processing ack: {}", e);
                    }
                },
                Ok(Message::Close(_)) => break,
                Ok(_) => break,
                Err(_) => break,
            }
        }
    }

    pub async fn send_data(&mut self, data: Vec<u8>, id: PackfileId) -> anyhow::Result<()> {
        let body = PackfileBody {
            header: Header {
                sequence_number: self.msg_counter,
                session_nonce: self.session_nonce,
            },
            id,
            data,
        };

        let body = bincode::serialize(&body)?;
        let signature = KEYS.get().unwrap().sign(&body).to_vec();
        let encapsulated = EncapsulatedPackfile { body, signature };

        let msg = bincode::serialize(&encapsulated)?;

        timeout(Duration::from_secs(PACKFILE_SEND_TIMEOUT), self.tx.send(Message::Binary(msg)))
            .await??;
        timeout(Duration::from_secs(PACKFILE_ACK_TIMEOUT), self.wait_for_ack(self.msg_counter))
            .await??;

        self.msg_counter += 1;
        Ok(())
    }

    pub async fn wait_for_ack(&mut self, sequence_number: u64) -> anyhow::Result<()> {
        let mut receiver = self.ack_notifier.subscribe();

        while let Ok(num) = receiver.recv().await {
            if num == sequence_number {
                return Ok(());
            }
        }

        bail!("Peer disconnected");
    }

    pub async fn done(mut self) {
        // try to close gracefully or ignore the error
        self.tx.close().await.ok();
    }
}
