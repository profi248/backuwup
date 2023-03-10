pub mod backup_request;
pub mod login;
pub mod register;

use anyhow::anyhow;
use poem::{error::ResponseError, http::StatusCode, Body, Request, Response};
use shared::{
    server_message::ServerMessage,
    types::{ClientId, SessionToken},
};

use crate::AUTH_MANAGER;

#[derive(thiserror::Error, Debug)]
#[error("error")]
pub struct ErrorWrapper(pub ServerMessage);

impl ResponseError for ErrorWrapper {
    fn status(&self) -> StatusCode {
        // todo change this
        StatusCode::OK
    }

    fn as_response(&self) -> Response {
        let body = Body::from_json(&self.0).expect("Failed to serialize response");
        Response::builder().status(self.status()).body(body)
    }
}

#[macro_export]
macro_rules! err_msg {
    ( $msg:expr ) => {
        $crate::handlers::ErrorWrapper(shared::server_message::ServerMessage::Error(
            shared::server_message::Error::Failure($msg.to_string()),
        ))
    };
}

pub fn check_token_header(request: &Request) -> anyhow::Result<ClientId> {
    let token = request
        .headers()
        .get("Authorization")
        .ok_or(anyhow!("Missing token"))?
        .to_str()?;

    check_token(token)
}

pub fn check_token(token: impl Into<String>) -> anyhow::Result<ClientId> {
    let token: SessionToken = hex::decode(token.into())?
        .try_into()
        .map_err(|_| anyhow!("Invalid token length"))?;

    let client = AUTH_MANAGER
        .get()
        .unwrap()
        .get_session(token)?
        .ok_or(anyhow!("Session doesn't exist"))?;

    Ok(client)
}
