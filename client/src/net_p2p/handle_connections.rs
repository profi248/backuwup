use std::time::Duration;

use anyhow::{anyhow, bail};
use ed25519_dalek::{PublicKey, Signature};
use futures_util::{SinkExt, StreamExt};
use shared::{
    p2p_message::{EncapsulateRequestBody, EncapsulatedMsg, Header, RequestType},
    server_message_ws::{FinalizeP2PConnection, IncomingP2PConnection},
    types::{ClientId, TransportSessionNonce},
};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{
    accept_async_with_config, connect_async_with_config, tungstenite, tungstenite::Message, MaybeTlsStream,
    WebSocketStream,
};

use crate::{
    backup::send,
    log,
    net_p2p::{get_listener_address, get_ws_config, received_files_writer},
    net_server::requests::p2p_connection_confirm,
    CONFIG, KEYS, TRANSPORT_REQUESTS,
};

pub async fn accept_and_listen(incoming_req: IncomingP2PConnection) -> anyhow::Result<()> {
    // reject connections from unknown peers
    if CONFIG
        .get()
        .unwrap()
        .get_peer_info(incoming_req.source_client_id)
        .await?
        .is_none()
    {
        log!(
            "[p2p] received a p2p connection request from an unknown peer {}, ignoring",
            hex::encode(incoming_req.source_client_id)
        );

        return Ok(());
    }

    // find our local IP address and generate a random port
    let (addr, port) = get_listener_address()?;

    let listener = TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    log!("[p2p] listening on port {}", port);

    // send our address/port in a confirmation message to the server
    p2p_connection_confirm(incoming_req.source_client_id, addr).await?;

    // wait for incoming connections
    let (stream, peer_addr) = listener.accept().await?;
    log!("[p2p] incoming connection from {}", peer_addr);

    // wrap the stream in this struct to have identical types in both connect functions
    let stream = MaybeTlsStream::Plain(stream);

    // upgrade the connection to a websocket, and wait for the client to send us a init message
    let mut stream = accept_async_with_config(stream, Some(get_ws_config())).await?;

    // finally, hand off the connection to the appropriate handler
    match receive_request(incoming_req.session_nonce, incoming_req.source_client_id, &mut stream).await? {
        // initiating peer wants to send new files to us
        RequestType::Transport => {
            received_files_writer::handle_receiving(
                incoming_req.source_client_id,
                incoming_req.session_nonce,
                stream,
            )
            .await?;
        }
        // initiating peer wants to restore from us
        RequestType::RestoreAll => {}
        _ => bail!("request type not implemented"),
    }

    Ok(())
}

pub async fn accept_and_connect(finalize_req: FinalizeP2PConnection) -> anyhow::Result<()> {
    let url = format!("ws://{}", finalize_req.destination_ip_address);

    // wait for the other party, trying to connect immediately usually fails
    tokio::time::sleep(Duration::from_secs(1)).await;

    log!("[p2p] trying to connect to peer at {}", url);
    let mut stream = sock_conn(&url).await?;
    log!("[p2p] connected successfully");

    let request = TRANSPORT_REQUESTS
        .get()
        .ok_or(anyhow!("transport requests not initialized"))?
        .finalize_request(finalize_req.destination_client_id)
        .await?;

    // as the requesting party, we are responsible for sending the initialization request message
    let body = bincode::serialize(&EncapsulateRequestBody {
        header: Header {
            sequence_number: 0,
            session_nonce: request.session_nonce,
        },
        request: request.purpose,
    })?;

    // sign the body
    let signature = KEYS.get().unwrap().sign(&body).to_vec();
    let msg = bincode::serialize(&EncapsulatedMsg { body, signature })?;

    stream.send(Message::from(msg)).await?;

    match request.purpose {
        RequestType::Transport => {
            send::connection_established(finalize_req.destination_client_id, request.session_nonce, stream)
                .await?;
        }
        RequestType::RestoreAll => {}
        _ => bail!("request type not implemented"),
    }

    Ok(())
}

async fn sock_conn(url: &String) -> anyhow::Result<WebSocketStream<MaybeTlsStream<TcpStream>>> {
    let mut attempts = 3;

    Ok(loop {
        // retry in case the socket takes a while to open
        match connect_async_with_config(url, Some(get_ws_config())).await {
            Ok((socket, _)) => break socket,
            Err(tungstenite::Error::Io(e)) => {
                if attempts > 0 {
                    log!("[p2p] peer connection failed: {}, will try {} more times", e, attempts);
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    attempts -= 1;
                    continue;
                } else {
                    bail!("unable to connect to peer after 3 retries: {e}")
                }
            }
            Err(e) => bail!(e),
        };
    })
}

async fn receive_request(
    nonce: TransportSessionNonce,
    client_id: ClientId,
    stream: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
) -> anyhow::Result<RequestType> {
    let data = match stream.next().await {
        Some(Ok(Message::Binary(data))) => data,
        Some(Ok(_)) => bail!("received invalid message type from peer"),
        Some(Err(e)) => bail!("error while waiting for request message: {}", e),
        None => bail!("peer closed connection before sending request message"),
    };

    // deserialize the message, and validate the authenticity
    let msg: EncapsulatedMsg = bincode::deserialize(&data)?;

    validate_encapsulated_signature(&client_id, &msg)?;
    let body: EncapsulateRequestBody = bincode::deserialize(&msg.body)?;

    if body.header.session_nonce != nonce || body.header.sequence_number != 0 {
        bail!("request message replay protection header invalid")
    }

    Ok(body.request)
}

fn validate_encapsulated_signature(
    source_pubkey: &[u8],
    encapsulated: &EncapsulatedMsg,
) -> anyhow::Result<()> {
    // verify signature on the bytes of the body
    let source_pubkey = PublicKey::from_bytes(source_pubkey)?;
    let signature = Signature::from_bytes(&encapsulated.signature)?;
    source_pubkey.verify_strict(&encapsulated.body, &signature)?;

    Ok(())
}
