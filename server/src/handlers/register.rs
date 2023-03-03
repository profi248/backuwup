use poem::{Request, web::Data, handler};
use poem::web::Json;
use crate::db::Database;
use shared::client_message::ClientRegistrationRequest;
use shared::server_message::{ClientRegistrationChallenge, Error, ServerMessage};

#[handler]
pub async fn register(
    Data(db): Data<&Database>,
    Json(request): Json<ClientRegistrationRequest>
) -> poem::Result<Json<ServerMessage>> {
    Ok(Json(ServerMessage::ClientRegistrationChallenge(ClientRegistrationChallenge {
        server_challenge: [0; 32],
    })))
}
