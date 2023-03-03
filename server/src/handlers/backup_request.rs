use poem::{
    handler,
    web::{Data, Json},
};
use shared::{
    client_message::BackupRequest,
    server_message::ServerMessage
};
use crate::backup_request::Queue;

#[handler]
pub async fn make_backup_request(
    Data(backup_requests): Data<&Queue>,
    Json(request): Json<BackupRequest>,
) -> poem::Result<Json<ServerMessage>> {
    Ok(Json(ServerMessage::Ok))
}
