use poem::{handler, web::Json};
use shared::{
    client_message::{ClientRegistrationAuth, ClientRegistrationRequest},
    server_message::{ClientRegistrationChallenge, ServerMessage},
    types::CHALLENGE_RESPONSE_LENGTH,
};

use crate::{handlers::Error, AUTH_MANAGER, DB};

#[handler]
pub async fn register_begin(
    Json(request): Json<ClientRegistrationRequest>,
) -> poem::Result<Json<ServerMessage>> {
    if DB.get().unwrap().client_exists(request.client_id).await? {
        return Err(Error::ClientExists(request.client_id).into());
    }

    let auth_manager = AUTH_MANAGER.get().unwrap();
    let server_challenge = auth_manager.challenge_begin(request.client_id)?;

    Ok(Json(ServerMessage::ClientRegistrationChallenge(ClientRegistrationChallenge { server_challenge })))
}

#[handler]
pub async fn register_complete(
    Json(request): Json<ClientRegistrationAuth>,
) -> poem::Result<Json<ServerMessage>> {
    // the response is passed in as Vec
    if request.challenge_response.len() != CHALLENGE_RESPONSE_LENGTH {
        return Err(Error::BadRequest.into());
    }

    let auth_manager = AUTH_MANAGER.get().unwrap();
    auth_manager.challenge_verify(request.client_id, &request.challenge_response)?;

    DB.get().unwrap().register_client(request.client_id).await?;

    println!("Client {} registered successfully", hex::encode(request.client_id));
    Ok(Json(ServerMessage::Ok))
}
