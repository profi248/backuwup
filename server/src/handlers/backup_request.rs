//! Handler for storage requests.

use poem::{handler, web::Json};
use shared::{client_message::BackupRequest, server_message::ServerMessage};

use crate::{backup_request::Request, handlers::Error, AUTH_MANAGER, BACKUP_REQUESTS};

#[handler]
pub async fn make_backup_request(Json(request): Json<BackupRequest>) -> poem::Result<Json<ServerMessage>> {
    let client_id = AUTH_MANAGER
        .get()
        .unwrap()
        .get_session(request.session_token)
        .ok_or(Error::Unauthorized)?;

    let request = Request {
        storage_required: request.storage_required,
        client_id,
    };

    let queue = BACKUP_REQUESTS.get().unwrap();

    println!("[backup request] new request: {request:?}");
    println!("\n[backup request] queue before fulfill vvv");
    queue.debug_print();

    match queue.fulfill(request).await {
        Ok(_) => {
            println!("\n[backup request] queue after fulfill vvv");
            queue.debug_print();

            Ok(Json(ServerMessage::Ok))
        }

        Err(e) => {
            println!("\n[backup request] fulfill failed");
            Err(e)?
        }
    }
}
