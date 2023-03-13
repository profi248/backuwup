use anyhow::{anyhow, bail};
use ed25519_dalek::{PublicKey, Signature};
use futures_util::StreamExt;
use portpicker::pick_unused_port;
use shared::{
    p2p_message::{BackupChunkBody, EncapsulatedBackupChunk, MAX_ENCAPSULATED_BACKUP_CHUNK_SIZE},
    types::{ClientId, TransportSessionNonce},
};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_async, tungstenite::Message};

use crate::LOGGER;

pub async fn listen(
    port: u16,
    session_nonce: TransportSessionNonce,
    source_pubkey: ClientId,
) -> anyhow::Result<()> {
    let listener = TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    LOGGER
        .get()
        .unwrap()
        .send(format!("[p2p] Listening on port {port}"));

    let (stream, peer_addr) = listener.accept().await?;

    LOGGER
        .get()
        .unwrap()
        .send(format!("[p2p] Incoming connection from {peer_addr}"));

    receive_handle_incoming(stream, session_nonce, source_pubkey)
        .await
        .unwrap();

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

async fn receive_handle_incoming(
    stream: TcpStream,
    session_nonce: TransportSessionNonce,
    source_pubkey: ClientId,
) -> anyhow::Result<()> {
    let mut stream = accept_async(stream).await?;

    let mut msg_counter: u64 = 0;
    loop {
        match stream.next().await {
            Some(Ok(msg)) => {
                let data =
                    validate_incoming_message(session_nonce, &source_pubkey, &mut msg_counter, msg);

                println!("{:?}", data?);
            }
            None => break,
            Some(Err(e)) => return Err(e.into()),
        }
    }

    Ok(())
}

fn validate_incoming_message(
    session_nonce: TransportSessionNonce,
    source_pubkey: &ClientId,
    msg_counter: &mut u64,
    msg: Message,
) -> anyhow::Result<Vec<u8>> {
    // drop the message if basic requirements are not met
    if !msg.is_binary() || msg.len() > MAX_ENCAPSULATED_BACKUP_CHUNK_SIZE {
        bail!("Protocol violation");
    }

    let encapsulated: EncapsulatedBackupChunk = bincode::deserialize(&msg.into_data())?;

    // verify signature on the bytes of the body
    let source_pubkey = PublicKey::from_bytes(source_pubkey)?;
    let signature = Signature::from_bytes(&encapsulated.signature)?;
    source_pubkey.verify_strict(&encapsulated.body, &signature)?;

    // decode the actual body
    let body: BackupChunkBody = bincode::deserialize(&encapsulated.body)?;

    // check header to enforce replay protection, random nonce has to match in the session,
    // and the sequence number has to be in order
    if body.session_nonce != session_nonce || body.sequence_number != *msg_counter {
        bail!("Couldn't verify message authenticity")
    }

    *msg_counter += 1;

    Ok(body.data)
}
