use anyhow::{anyhow, bail};
use ed25519_dalek::{PublicKey, Signature};
use futures_util::{SinkExt, StreamExt};
use portpicker::pick_unused_port;
use shared::{
    p2p_message::{
        AckBody, EncapsulatedFile, EncapsulatedFileBody, EncapsulatedPackfileAck, FileInfo, Header,
    },
    types::{ClientId, TransportSessionNonce},
};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_async_with_config, tungstenite::Message};

use crate::{
    log,
    net_p2p::{get_ws_config, received_files_writer::Receiver},
    KEYS, UI,
};

pub async fn listen(
    port: u16,
    session_nonce: TransportSessionNonce,
    source_pubkey: ClientId,
    receiver: Receiver,
) -> anyhow::Result<()> {
    let listener = TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    log!("[p2p] Listening on port {}", port);

    let (stream, peer_addr) = listener.accept().await?;

    log!("[p2p] Incoming connection from {}", peer_addr);

    receive_handle_incoming(stream, session_nonce, source_pubkey, receiver)
        .await
        .map_err(|e| {
            log!("[p2p] Connection failed: {}", e);
        })
        .ok();

    Ok(())
}

pub fn get_listener_address() -> anyhow::Result<(String, u16)> {
    // get our IP address on the local network, since we can currently only connect to
    // computers in the same network segment
    let local_ip_addr = local_ip_address::local_ip()?;

    // try picking an unused high range port
    let port = pick_unused_port().ok_or(anyhow!("Unable to pick an unused port"))?;

    Ok((format!("{local_ip_addr}:{port}"), port))
}

fn ack_msg(
    nonce: TransportSessionNonce,
    seq: &mut u64,
    acknowledged: u64,
) -> anyhow::Result<Message> {
    let body = bincode::serialize(&AckBody {
        header: Header {
            sequence_number: *seq,
            session_nonce: nonce,
        },
        acknowledged_sequence_number: acknowledged,
    })?;

    let signature = KEYS.get().unwrap().sign(&body).to_vec();
    let msg = EncapsulatedPackfileAck { body, signature };

    *seq += 1;

    Ok(Message::Binary(bincode::serialize(&msg)?))
}

async fn receive_handle_incoming(
    stream: TcpStream,
    session_nonce: TransportSessionNonce,
    source_pubkey: ClientId,
    receiver: Receiver,
) -> anyhow::Result<()> {
    let mut stream = accept_async_with_config(stream, Some(get_ws_config())).await?;

    let mut data_msg_counter: u64 = 0;
    let mut ack_msg_counter: u64 = 0;

    loop {
        match stream.next().await {
            Some(Ok(Message::Binary(msg))) => {
                let (msg_num, file_info, mut data) = validate_incoming_message(
                    session_nonce,
                    &source_pubkey,
                    &mut data_msg_counter,
                    &msg,
                )?;

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
            Some(Ok(_)) => return Err(anyhow!("Invalid message type received")),
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
    let encapsulated: EncapsulatedFile = bincode::deserialize(encapsulated_data)?;

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
