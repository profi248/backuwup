use poem::{Request, web::Data, handler, IntoResponse};
use poem::web::Json;
use shared::client_message::{ClientRegistrationAuth, ClientRegistrationRequest};
use shared::server_message::{ClientRegistrationChallenge, ServerMessage};
use shared::server_message::Error::BadRequest;
use crate::Challenges;
use crate::handlers::ServerResponse;

#[handler]
pub async fn register_begin(
    Data(challenges): Data<&Challenges>,
    Json(request): Json<ClientRegistrationRequest>
) -> poem::Result<Json<ServerMessage>> {
    challenges.lock().expect("Failed to acquire lock on challenges cache in register")
        .insert(request.client_id, [0; 32]);

    // todo check if client id is already registered

    Ok(Json(ServerMessage::ClientRegistrationChallenge(ClientRegistrationChallenge {
        server_challenge: [0; 32],
    })))
}

#[handler]
pub async fn register_complete(
    Data(challenges): Data<&Challenges>,
    Json(request): Json<ClientRegistrationAuth>
) -> poem::Result<Json<ServerMessage>> {
    challenges.lock().expect("Failed to acquire lock on challenges cache in register")
        .get(&request.client_id).ok_or(
        ServerResponse(ServerMessage::Error(BadRequest("Invalid client id".to_string())))
    )?;

    // todo check challenge and add client to db

    Ok(Json(ServerMessage::Ok))
}

