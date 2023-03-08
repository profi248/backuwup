use poem::{handler, web::Json};
use shared::{
    client_message::{ClientLoginAuth, ClientLoginRequest},
    server_message::{ClientLoginChallenge, ClientLoginToken, Error, ServerMessage},
    types::CHALLENGE_RESPONSE_LENGTH,
};

use crate::{handlers::ServerResponse, AUTH_MANAGER};

#[handler]
pub async fn login_begin(
    Json(request): Json<ClientLoginRequest>,
) -> poem::Result<Json<ServerMessage>> {
    let auth_manager = AUTH_MANAGER.get().expect("OnceCell failed");

    let server_challenge = auth_manager
        .challenge_begin(request.client_id)
        .map_err(|e| ServerResponse(ServerMessage::Error(Error::Failure(e.to_string()))))?;

    // todo check if client id is already registered

    Ok(Json(ServerMessage::ClientLoginChallenge(ClientLoginChallenge { server_challenge })))
}

#[handler]
pub fn login_complete(Json(request): Json<ClientLoginAuth>) -> poem::Result<Json<ServerMessage>> {
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

    let token = auth_manager
        .session_start(request.client_id)
        .map_err(|e| ServerResponse(ServerMessage::Error(Error::Failure(e.to_string()))))?;

    Ok(Json(ServerMessage::ClientLoginToken(ClientLoginToken { token })))
}
