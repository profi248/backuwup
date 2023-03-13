use futures_util::SinkExt;
use shared::{
    p2p_message::{BackupChunkBody, EncapsulatedBackupChunk},
    types::TransportSessionNonce,
};
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

use crate::KEYS;

pub struct BackupTransportManager {
    socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
    msg_counter: u64,
    session_nonce: TransportSessionNonce,
}

impl BackupTransportManager {
    pub async fn new(addr: String, session_nonce: TransportSessionNonce) -> anyhow::Result<Self> {
        let url = format!("ws://{addr}");
        let (socket, _) = connect_async(url).await?;

        Ok(BackupTransportManager {
            socket,
            msg_counter: 0,
            session_nonce,
        })
    }

    pub async fn send_data(&mut self, data: Vec<u8>) -> anyhow::Result<()> {
        let body = BackupChunkBody {
            sequence_number: self.msg_counter,
            session_nonce: self.session_nonce,
            data,
        };

        self.msg_counter += 1;

        let body = bincode::serialize(&body)?;
        let signature = KEYS.get().unwrap().sign(&body).to_vec();

        let encapsulated = EncapsulatedBackupChunk { body, signature };

        let msg = bincode::serialize(&encapsulated)?;
        self.socket.send(Message::Binary(msg)).await?;

        Ok(())
    }

    pub async fn done(mut self) -> anyhow::Result<()> {
        self.socket.close(None).await?;
        Ok(())
    }
}
