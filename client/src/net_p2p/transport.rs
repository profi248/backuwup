//! Implements the sending part of the P2P transport protocol, including replay protection and
//! acknowledgements.

use std::time::Duration;

use anyhow::bail;
use ed25519_dalek::{PublicKey, Signature};
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use shared::{
    p2p_message::{AckBody, EncapsulatedFileBody, EncapsulatedMsg, FileInfo, Header},
    types::{ClientId, TransportSessionNonce},
};
use tokio::{net::TcpStream, sync::broadcast, time::timeout};
use tokio_tungstenite::{tungstenite::Message, MaybeTlsStream, WebSocketStream};

use crate::{
    defaults::{PACKFILE_ACK_TIMEOUT, PACKFILE_SEND_TIMEOUT},
    log, KEYS,
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
    pub fn new(
        socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
        session_nonce: TransportSessionNonce,
        client_id: ClientId,
    ) -> Self {
        let (tx, rx) = socket.split();
        let (ack_sender, _) = broadcast::channel(100);

        BackupTransportManager {
            tx,
            // start the counter at 1, since 0 is reserved for the init message
            msg_counter: 1,
            session_nonce,
            ack_notifier: ack_sender.clone(),
            ack_handle: tokio::spawn(Self::process_acks(rx, ack_sender, session_nonce, client_id)),
        }
    }

    pub fn parse_incoming_ack(
        data: &[u8],
        nonce: TransportSessionNonce,
        sender_pubkey: ClientId,
        messages_received: &mut u64,
    ) -> anyhow::Result<u64> {
        let encapsulated: EncapsulatedMsg = bincode::deserialize(data)?;

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
                Ok(Message::Binary(data)) => {
                    match Self::parse_incoming_ack(&data, nonce, sender_pubkey, &mut messages_received) {
                        Ok(message_id) => {
                            notifier.send(message_id).ok();
                        }
                        Err(e) => {
                            log!("[p2p] error while processing ack: {e}");
                        }
                    }
                }
                Ok(Message::Close(_)) => break,
                Ok(_) => log!("[p2p] invalid message type while waiting for ack"),
                Err(e) => log!("[p2p] WebSocket error while waiting for ack: {e}"),
            }
        }
    }

    pub async fn send_data(&mut self, data: Vec<u8>, file_info: FileInfo) -> anyhow::Result<()> {
        let body = EncapsulatedFileBody {
            header: Header {
                sequence_number: self.msg_counter,
                session_nonce: self.session_nonce,
            },
            file_info,
            data,
        };

        let body = bincode::serialize(&body)?;
        let signature = KEYS.get().unwrap().sign(&body).to_vec();
        let encapsulated = EncapsulatedMsg { body, signature };

        let msg = bincode::serialize(&encapsulated)?;

        timeout(Duration::from_secs(PACKFILE_SEND_TIMEOUT), self.tx.send(Message::Binary(msg))).await??;
        timeout(Duration::from_secs(PACKFILE_ACK_TIMEOUT), self.wait_for_ack(self.msg_counter)).await??;

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
