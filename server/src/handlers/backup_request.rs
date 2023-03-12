use poem::{handler, web::Json};
use shared::{client_message::BackupRequest, server_message::ServerMessage};

use crate::BACKUP_REQUESTS;

#[handler]
pub async fn make_backup_request(
    Json(request): Json<BackupRequest>,
) -> poem::Result<Json<ServerMessage>> {
    // todo verify client token
    let request = request.into();
    let queue = BACKUP_REQUESTS.get().unwrap();

    println!("[backup request] new request: {request:?}");
    println!("\n[backup request] queue before fulfill vvv");
    queue.debug_print();

    match queue.fulfill(request).await {
        Ok(_) => {
            println!("\n[backup request] queue after fulfill vvv");
            queue.debug_print();

            // todo provide more details
            Ok(Json(ServerMessage::Ok))
        }

        Err(e) => {
            println!("\n[backup request] fulfill failed");
            Err(e)?
        }
    }
}
