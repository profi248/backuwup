use poem::{
    handler,
    web::Json,
};
use poem::web::Data;
use shared::client_message::BeginTransportRequest;
use shared::server_message::ServerMessage;
use shared::server_message_ws::{IncomingTransportRequest, ServerMessageWs};
use crate::{auth_err, AUTH_MANAGER, CONNECTIONS, err_msg};
use crate::db::Database;

#[handler]
pub async fn transport_begin(
    Json(request): Json<BeginTransportRequest>,
    Data(db): Data<&Database>,
) -> poem::Result<Json<ServerMessage>> {
    let source_client_id =
        AUTH_MANAGER.get().unwrap().get_session(request.session_token)
        .map_err(|e| err_msg!(e))?
        .ok_or(auth_err!())?;

    let destination_client_id = request.destination_client_id;
    let session_nonce = request.session_nonce;

    if db.client_exists(destination_client_id).await
        .map_err(|e| err_msg!(e))?
    {
        Err(err_msg!("destination client does not exist"))?;
    }

    CONNECTIONS.get().unwrap().notify_client(destination_client_id,
        ServerMessageWs::IncomingTransportRequest(
            IncomingTransportRequest {
                source_client_id,
                session_nonce,
            })
    ).await.map_err(|_| err_msg!("Unable to notify destination client"))?;

    Ok(Json(ServerMessage::Ok))
}
