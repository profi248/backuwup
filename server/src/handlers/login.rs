//! Handlers for login API endpoints.

use poem::{handler, web::Json};
use shared::{
    client_message::{ClientLoginAuth, ClientLoginRequest},
    server_message::{ClientLoginChallenge, ClientLoginToken, ServerMessage},
    types::CHALLENGE_RESPONSE_LENGTH,
};

use crate::{handlers, AUTH_MANAGER, DB};

#[handler]
pub async fn login_begin(Json(request): Json<ClientLoginRequest>) -> poem::Result<Json<ServerMessage>> {
    if !DB.get().unwrap().client_exists(request.client_id).await? {
        Err(handlers::Error::ClientNotFound(request.client_id))?;
    }

    let auth_manager = AUTH_MANAGER.get().unwrap();
    let server_challenge = auth_manager.challenge_begin(request.client_id)?;

    Ok(Json(ServerMessage::ClientLoginChallenge(ClientLoginChallenge { server_challenge })))
}

#[handler]
pub async fn login_complete(Json(request): Json<ClientLoginAuth>) -> poem::Result<Json<ServerMessage>> {
    // the response is passed in as Vec
    if request.challenge_response.len() != CHALLENGE_RESPONSE_LENGTH {
        Err(handlers::Error::BadRequest)?;
    }

    let auth_manager = AUTH_MANAGER.get().unwrap();
    auth_manager.challenge_verify(request.client_id, &request.challenge_response)?;

    let token = auth_manager.session_start(request.client_id)?;

    DB.get().unwrap().client_update_logged_in(request.client_id).await?;

    Ok(Json(ServerMessage::ClientLoginToken(ClientLoginToken { token })))
}
