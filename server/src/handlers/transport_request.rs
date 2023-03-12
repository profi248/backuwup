use poem::{
    handler,
    web::{Data, Json},
};
use shared::{
    client_message::BeginTransportRequest,
    server_message::ServerMessage,
    server_message_ws::{IncomingTransportRequest, ServerMessageWs},
};

use crate::{
    db::Database,
    handlers::{Error, Error::ClientNotFound},
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

    if db.client_exists(destination_client_id).await? {
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
