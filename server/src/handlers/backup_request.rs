use poem::{
    handler,
    web::{Data, Json},
};
use shared::{
    client_message::BackupRequest,
    server_message::ServerMessage
};
use crate::backup_request::{Queue, Request};
use crate::BACKUP_REQUESTS;

#[handler]
pub async fn make_backup_request(
    Json(request): Json<BackupRequest>,
) -> poem::Result<Json<ServerMessage>> {
    // todo verify client token
    let request = request.into();
    let queue = BACKUP_REQUESTS.get().expect("OnceCell failed");

    match queue.fulfill(request).await {
        Ok(_) => {}
        Err(_) => {}
    }

    Ok(Json(ServerMessage::Ok))
}
