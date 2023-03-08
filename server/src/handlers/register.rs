use poem::{handler, web::Json};
use shared::{
    client_message::{ClientRegistrationAuth, ClientRegistrationRequest},
    server_message::{ClientRegistrationChallenge, Error, ServerMessage},
    types::CHALLENGE_RESPONSE_LENGTH,
};

use crate::{handlers::ServerResponse, AUTH_MANAGER};

#[handler]
pub async fn register_begin(
    Json(request): Json<ClientRegistrationRequest>,
) -> poem::Result<Json<ServerMessage>> {
    let auth_manager = AUTH_MANAGER.get().expect("OnceCell failed");

    auth_manager
        .challenge_begin(request.client_id)
        .map_err(|e| ServerResponse(ServerMessage::Error(Error::Failure(e.to_string()))))?;

    // todo check if client id is already registered

    Ok(Json(ServerMessage::ClientRegistrationChallenge(ClientRegistrationChallenge {
        server_challenge: [0; 16],
    })))
}

#[handler]
pub async fn register_complete(
    Json(request): Json<ClientRegistrationAuth>,
) -> poem::Result<Json<ServerMessage>> {
    // the response is passed in as Vec
    if request.challenge_response.len() != CHALLENGE_RESPONSE_LENGTH {
        Err(ServerResponse(ServerMessage::Error(Error::Failure(
            "Challenge response is invalid length".to_string(),
        ))))?;
    }

    let auth_manager = AUTH_MANAGER.get().expect("OnceCell failed");

    auth_manager
        .challenge_verify(request.client_id, request.challenge_response)
        .map_err(|e| ServerResponse(ServerMessage::Error(Error::Failure(e.to_string()))))?;

    // todo check challenge and add client to db

    Ok(Json(ServerMessage::Ok))
}
