use poem::{handler, web::Json};
use shared::{
    client_message::{ClientRegistrationAuth, ClientRegistrationRequest},
    server_message::{ClientRegistrationChallenge, Error, Error::Failure, ServerMessage},
};

use crate::{handlers::ServerResponse, AUTH_MANAGER};

#[handler]
pub async fn register_begin(
    Json(request): Json<ClientRegistrationRequest>,
) -> poem::Result<Json<ServerMessage>> {
    AUTH_MANAGER
        .get()
        .expect("OnceCell failed")
        .challenge_begin(request.client_id)
        .await
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
    if request.challenge_response.len() != 64 {
        Err(ServerResponse(ServerMessage::Error(Failure(
            "Challenge response is invalid length".to_string(),
        ))))?;
    }

    AUTH_MANAGER
        .get()
        .expect("OnceCell failed")
        .challenge_verify(request.client_id, request.challenge_response)
        .await
        .map_err(|e| ServerResponse(ServerMessage::Error(Error::Failure(e.to_string()))))?;

    // todo check challenge and add client to db

    Ok(Json(ServerMessage::Ok))
}
