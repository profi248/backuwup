use crate::handlers::ServerResponse;
use crate::Challenges;
use poem::web::Json;
use poem::{handler, web::Data, IntoResponse, Request};
use shared::{
    client_message::{ClientRegistrationAuth, ClientRegistrationRequest},
    server_message::{ClientRegistrationChallenge, Error, ServerMessage},
};

#[handler]
pub async fn register_begin(
    Data(challenges): Data<&Challenges>,
    Json(request): Json<ClientRegistrationRequest>,
) -> poem::Result<Json<ServerMessage>> {
    challenges
        .lock()
        .expect("Failed to acquire lock on challenges cache in register")
        .insert(request.client_id, [0; 32]);

    // todo check if client id is already registered

    Ok(Json(ServerMessage::ClientRegistrationChallenge(
        ClientRegistrationChallenge {
            server_challenge: [0; 32],
        },
    )))
}

#[handler]
pub async fn register_complete(
    Data(challenges): Data<&Challenges>,
    Json(request): Json<ClientRegistrationAuth>,
) -> poem::Result<Json<ServerMessage>> {
    challenges
        .lock()
        .expect("Failed to acquire lock on challenges cache in register")
        .get(&request.client_id)
        .ok_or(ServerResponse(ServerMessage::Error(Error::BadRequest(
            "Invalid client id".to_string(),
        ))))?;

    // todo check challenge and add client to db

    Ok(Json(ServerMessage::Ok))
}
