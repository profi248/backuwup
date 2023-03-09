use poem::{
    handler,
    web::{Data, Json},
};
use shared::{
    client_message::{ClientRegistrationAuth, ClientRegistrationRequest},
    server_message::{ClientRegistrationChallenge, ServerMessage},
    types::CHALLENGE_RESPONSE_LENGTH,
};

use crate::{db::Database, err_msg, AUTH_MANAGER};

#[handler]
pub async fn register_begin(
    Json(request): Json<ClientRegistrationRequest>,
    Data(db): Data<&Database>,
) -> poem::Result<Json<ServerMessage>> {
    if db
        .client_exists(request.client_id)
        .await
        .map_err(|e| err_msg!(e))?
    {
        Err(err_msg!("Client with this public key is already registered"))?;
    }

    let auth_manager = AUTH_MANAGER.get().expect("OnceCell failed");
    let server_challenge = auth_manager
        .challenge_begin(request.client_id)
        .map_err(|e| err_msg!(e))?;

    Ok(Json(ServerMessage::ClientRegistrationChallenge(ClientRegistrationChallenge {
        server_challenge,
    })))
}

#[handler]
pub async fn register_complete(
    Json(request): Json<ClientRegistrationAuth>,
    Data(db): Data<&Database>,
) -> poem::Result<Json<ServerMessage>> {
    // the response is passed in as Vec
    if request.challenge_response.len() != CHALLENGE_RESPONSE_LENGTH {
        Err(err_msg!("Challenge response is invalid length"))?;
    }

    let auth_manager = AUTH_MANAGER.get().expect("OnceCell failed");

    auth_manager
        .challenge_verify(request.client_id, request.challenge_response)
        .map_err(|e| err_msg!(e))?;

    db.register_client(request.client_id)
        .await
        .map_err(|e| err_msg!(e))?;

    Ok(Json(ServerMessage::Ok))
}
