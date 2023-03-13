use poem::{
    handler,
    web::{Data, Json},
};
use shared::{
    client_message::{ClientRegistrationAuth, ClientRegistrationRequest},
    server_message::{ClientRegistrationChallenge, ServerMessage},
    types::CHALLENGE_RESPONSE_LENGTH,
};

use crate::{db::Database, handlers::Error, AUTH_MANAGER};

#[handler]
pub async fn register_begin(
    Json(request): Json<ClientRegistrationRequest>,
    Data(db): Data<&Database>,
) -> poem::Result<Json<ServerMessage>> {
    if db.client_exists(request.client_id).await? {
        return Err(Error::ClientExists(request.client_id).into());
    }

    let auth_manager = AUTH_MANAGER.get().unwrap();
    let server_challenge = auth_manager.challenge_begin(request.client_id)?;

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
        return Err(Error::BadRequest.into());
    }

    let auth_manager = AUTH_MANAGER.get().unwrap();
    auth_manager.challenge_verify(request.client_id, &request.challenge_response)?;

    db.register_client(request.client_id).await?;

    // todo nicer print formatting
    println!("Client {:?} registered successfully", request.client_id);
    Ok(Json(ServerMessage::Ok))
}
