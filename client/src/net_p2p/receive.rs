use anyhow::{anyhow, bail};
use ed25519_dalek::{PublicKey, Signature};
use futures_util::{SinkExt, StreamExt};
use shared::{
    p2p_message::{AckBody, EncapsulatedFileBody, EncapsulatedMsg, FileInfo, Header},
    types::{ClientId, PackfileId, TransportSessionNonce},
};
use tokio::net::TcpStream;
use tokio_tungstenite::{tungstenite::Message, MaybeTlsStream, WebSocketStream};

use crate::{KEYS, UI};

#[async_trait::async_trait]
pub trait Receiver {
    async fn save_index(&self, id: u32, data: &mut [u8]) -> anyhow::Result<()>;
    async fn save_packfile(&self, id: PackfileId, data: &mut [u8]) -> anyhow::Result<()>;
}

fn ack_msg(nonce: TransportSessionNonce, seq: &mut u64, acknowledged: u64) -> anyhow::Result<Message> {
    let body = bincode::serialize(&AckBody {
        header: Header { sequence_number: *seq, session_nonce: nonce },
        acknowledged_sequence_number: acknowledged,
    })?;

    let signature = KEYS.get().unwrap().sign(&body).to_vec();
    let msg = EncapsulatedMsg { body, signature };

    *seq += 1;

    Ok(Message::Binary(bincode::serialize(&msg)?))
}

pub async fn handle_stream(
    mut stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
    session_nonce: TransportSessionNonce,
    source_pubkey: ClientId,
    receiver: impl Receiver,
) -> anyhow::Result<()> {
    // start the counter at 1, because 0 was used by the initiation message
    let mut data_msg_counter: u64 = 1;
    let mut ack_msg_counter: u64 = 0;

    loop {
        match stream.next().await {
            Some(Ok(Message::Binary(msg))) => {
                let (msg_num, file_info, mut data) =
                    validate_incoming_message(session_nonce, &source_pubkey, &mut data_msg_counter, &msg)?;

                println!("[p2p] received file {:?}", file_info);

                match file_info {
                    FileInfo::Packfile(id) => receiver.save_packfile(id, &mut data).await?,
                    FileInfo::Index(id) => receiver.save_index(id, &mut data).await?,
                }

                stream
                    .send(ack_msg(session_nonce, &mut ack_msg_counter, msg_num)?)
                    .await?;
            }
            Some(Ok(Message::Close(_))) | None => {
                UI.get().unwrap().log("[p2p] transport finished");
                break;
            }
            Some(Ok(_)) => return Err(anyhow!("invalid message type received")),
            Some(Err(e)) => return Err(e.into()),
        }
    }

    Ok(())
}

fn validate_incoming_message(
    session_nonce: TransportSessionNonce,
    source_pubkey: &ClientId,
    msg_counter: &mut u64,
    encapsulated_data: &[u8],
) -> anyhow::Result<(u64, FileInfo, Vec<u8>)> {
    let encapsulated: EncapsulatedMsg = bincode::deserialize(encapsulated_data)?;

    // verify signature on the bytes of the body
    let source_pubkey = PublicKey::from_bytes(source_pubkey)?;
    let signature = Signature::from_bytes(&encapsulated.signature)?;
    source_pubkey.verify_strict(&encapsulated.body, &signature)?;

    // decode the actual body
    let body: EncapsulatedFileBody = bincode::deserialize(&encapsulated.body)?;

    // check header to enforce replay protection, random nonce has to match in the session,
    // and the sequence number has to be in order
    if body.header.session_nonce != session_nonce || body.header.sequence_number != *msg_counter {
        bail!("Couldn't verify message authenticity")
    }

    *msg_counter += 1;

    Ok((body.header.sequence_number, body.file_info, body.data))
}
