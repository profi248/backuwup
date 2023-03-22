use poem::{
    handler,
    web::{Data, Json},
};
use shared::{
    client_message::{BeginTransportRequest, ConfirmTransportRequest},
    server_message::ServerMessage,
    server_message_ws::{FinalizeTransportRequest, IncomingTransportRequest, ServerMessageWs},
};

use crate::{
    db::Database,
    handlers::{
        Error,
        Error::{BadRequest, ClientNotFound},
    },
    AUTH_MANAGER, CONNECTIONS,
};

#[handler]
pub async fn transport_begin(
    Json(request): Json<BeginTransportRequest>,
    Data(db): Data<&Database>,
) -> poem::Result<Json<ServerMessage>> {
    let source_client_id = AUTH_MANAGER
        .get()
        .unwrap()
        .get_session(request.session_token)
        .ok_or(Error::Unauthorized)?;

    let destination_client_id = request.destination_client_id;
    let session_nonce = request.session_nonce;

    println!("backup transport begin request received to {destination_client_id:?}");

    if !db.client_exists(destination_client_id).await? {
        println!("{destination_client_id:?} doesn't exist");
        Err(ClientNotFound(destination_client_id))?;
    }

    CONNECTIONS
        .get()
        .unwrap()
        .notify_client(
            destination_client_id,
            ServerMessageWs::IncomingTransportRequest(IncomingTransportRequest {
                source_client_id,
                session_nonce,
            }),
        )
        .await?;

    Ok(Json(ServerMessage::Ok))
}

#[handler]
pub async fn transport_confirm(
    Json(request): Json<ConfirmTransportRequest>,
    Data(db): Data<&Database>,
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

    if !db.client_exists(source_client_id).await? {
        return Err(ClientNotFound(source_client_id).into());
    }

    CONNECTIONS
        .get()
        .unwrap()
        .notify_client(
            source_client_id,
            ServerMessageWs::FinalizeTransportRequest(FinalizeTransportRequest {
                destination_client_id,
                destination_ip_address,
            }),
        )
        .await?;

    Ok(Json(ServerMessage::Ok))
}
