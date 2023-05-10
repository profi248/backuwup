//! Handlers for P2P connection establishment API endpoints.

use poem::{handler, web::Json};
use shared::{
    client_message::{BeginP2PConnectionRequest, ConfirmP2PConnectionRequest},
    server_message::ServerMessage,
    server_message_ws::{FinalizeP2PConnection, IncomingP2PConnection, ServerMessageWs},
};

use crate::{
    handlers::{
        Error,
        Error::{BadRequest, ClientNotFound},
    },
    AUTH_MANAGER, CONNECTIONS, DB,
};

/// Handler for the P2P connection begin request.
#[handler]
pub async fn p2p_connection_begin(
    Json(request): Json<BeginP2PConnectionRequest>,
) -> poem::Result<Json<ServerMessage>> {
    let source_client_id = AUTH_MANAGER
        .get()
        .unwrap()
        .get_session(request.session_token)
        .ok_or(Error::Unauthorized)?;

    let destination_client_id = request.destination_client_id;
    let session_nonce = request.session_nonce;

    println!("p2p connection begin request received to {}", hex::encode(destination_client_id));

    if !DB.get().unwrap().client_exists(destination_client_id).await? {
        println!("{destination_client_id:?} doesn't exist");
        Err(ClientNotFound(destination_client_id))?;
    }

    CONNECTIONS
        .get()
        .unwrap()
        .notify_client(
            destination_client_id,
            ServerMessageWs::IncomingP2PConnection(IncomingP2PConnection { source_client_id, session_nonce }),
        )
        .await?;

    Ok(Json(ServerMessage::Ok))
}

/// Handler for the P2P connection confirm request.
#[handler]
pub async fn p2p_connection_confirm(
    Json(request): Json<ConfirmP2PConnectionRequest>,
) -> poem::Result<Json<ServerMessage>> {
    let destination_client_id = AUTH_MANAGER
        .get()
        .unwrap()
        .get_session(request.session_token)
        .ok_or(Error::Unauthorized)?;

    let source_client_id = request.source_client_id;
    let destination_ip_address = request.destination_ip_address;

    if destination_ip_address.len() > 64 {
        return Err(BadRequest.into());
    }

    println!("p2p connection confirm request received to {}", hex::encode(destination_client_id));

    if !DB.get().unwrap().client_exists(source_client_id).await? {
        return Err(ClientNotFound(source_client_id).into());
    }

    CONNECTIONS
        .get()
        .unwrap()
        .notify_client(
            source_client_id,
            ServerMessageWs::FinalizeP2PConnection(FinalizeP2PConnection {
                destination_client_id,
                destination_ip_address,
            }),
        )
        .await?;

    Ok(Json(ServerMessage::Ok))
}
