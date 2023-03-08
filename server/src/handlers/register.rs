use poem::{
    handler,
    web::{Data, Json},
};
use shared::{
    client_message::{ClientRegistrationAuth, ClientRegistrationRequest},
    server_message::{ClientRegistrationChallenge, Error, ServerMessage},
    types::CHALLENGE_RESPONSE_LENGTH,
};

use crate::{db::Database, handlers::ErrorWrapper, AUTH_MANAGER};

#[handler]
pub async fn register_begin(
    Json(request): Json<ClientRegistrationRequest>,
    Data(db): Data<&Database>,
) -> poem::Result<Json<ServerMessage>> {
    let auth_manager = AUTH_MANAGER.get().expect("OnceCell failed");

    if db
        .client_exists(request.client_id)
        .await
        .map_err(|e| ErrorWrapper(ServerMessage::Error(Error::Failure(e.to_string()))))?
    {
        Err(ErrorWrapper(ServerMessage::Error(Error::Failure(
            "Client with this public key is already registered".to_string(),
        ))))?;
    }

    db.register_client(request.client_id)
        .await
        .map_err(|e| ErrorWrapper(ServerMessage::Error(Error::Failure(e.to_string()))))?;

    auth_manager
        .challenge_begin(request.client_id)
        .map_err(|e| ErrorWrapper(ServerMessage::Error(Error::Failure(e.to_string()))))?;

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
        Err(ErrorWrapper(ServerMessage::Error(Error::Failure(
            "Challenge response is invalid length".to_string(),
        ))))?;
    }

    let auth_manager = AUTH_MANAGER.get().expect("OnceCell failed");

    auth_manager
        .challenge_verify(request.client_id, request.challenge_response)
        .map_err(|e| ErrorWrapper(ServerMessage::Error(Error::Failure(e.to_string()))))?;

    // todo check challenge and add client to db

    Ok(Json(ServerMessage::Ok))
}
