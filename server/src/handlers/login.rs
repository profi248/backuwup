use poem::{
    handler,
    web::{Data, Json},
};
use shared::{
    client_message::{ClientLoginAuth, ClientLoginRequest},
    server_message::{ClientLoginChallenge, ClientLoginToken, ServerMessage},
    types::CHALLENGE_RESPONSE_LENGTH,
};

use crate::{db::Database, err_msg, AUTH_MANAGER};

#[handler]
pub async fn login_begin(
    Json(request): Json<ClientLoginRequest>,
    Data(db): Data<&Database>,
) -> poem::Result<Json<ServerMessage>> {
    if !db
        .client_exists(request.client_id)
        .await
        .map_err(|e| err_msg!(e))?
    {
        Err(err_msg!("Client with this public key is not registered"))?;
    }

    let auth_manager = AUTH_MANAGER.get().unwrap();
    let server_challenge = auth_manager
        .challenge_begin(request.client_id)
        .map_err(|e| err_msg!(e.to_string()))?;

    Ok(Json(ServerMessage::ClientLoginChallenge(ClientLoginChallenge { server_challenge })))
}

#[handler]
pub fn login_complete(Json(request): Json<ClientLoginAuth>) -> poem::Result<Json<ServerMessage>> {
    // the response is passed in as Vec
    if request.challenge_response.len() != CHALLENGE_RESPONSE_LENGTH {
        Err(err_msg!("Challenge response is invalid length"))?;
    }

    let auth_manager = AUTH_MANAGER.get().unwrap();
    auth_manager
        .challenge_verify(request.client_id, request.challenge_response)
        .map_err(|e| err_msg!(e))?;

    let token = auth_manager
        .session_start(request.client_id)
        .map_err(|e| err_msg!(e))?;

    Ok(Json(ServerMessage::ClientLoginToken(ClientLoginToken { token })))
}
