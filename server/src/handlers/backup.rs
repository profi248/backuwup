//! Handlers for for backup/snapshot-related API endpoints.

use poem::{handler, web::Json};
use shared::{
    client_message::{BackupDone, BackupRestoreRequest},
    server_message::{BackupRestoreInfo, ServerMessage},
};

use crate::{handlers::Error, AUTH_MANAGER, DB};

#[handler]
pub async fn backup_done(Json(request): Json<BackupDone>) -> poem::Result<Json<ServerMessage>> {
    let source_client_id = AUTH_MANAGER
        .get()
        .unwrap()
        .get_session(request.session_token)
        .ok_or(Error::Unauthorized)?;

    DB.get()
        .unwrap()
        .save_snapshot(source_client_id, request.snapshot_hash)
        .await?;

    Ok(Json(ServerMessage::Ok))
}

#[handler]
pub async fn backup_restore(Json(request): Json<BackupRestoreRequest>) -> poem::Result<Json<ServerMessage>> {
    let source_client_id = AUTH_MANAGER
        .get()
        .unwrap()
        .get_session(request.session_token)
        .ok_or(Error::Unauthorized)?;

    let db = DB.get().unwrap();

    let snapshot_hash = db.get_latest_client_snapshot(source_client_id).await?;
    if snapshot_hash.is_none() {
        Err(Error::NoBackupsAvailable)?;
    }

    let message = BackupRestoreInfo {
        snapshot_hash: snapshot_hash.unwrap(),
        peers: db.get_client_negotiated_peers(source_client_id).await?,
    };

    Ok(Json(ServerMessage::BackupRestoreInfo(message)))
}
