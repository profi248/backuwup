pub mod backup_request;
pub mod login;
pub mod register;
pub mod transport_request;

use anyhow::anyhow;
use poem::{error::ResponseError, http::StatusCode, Body, Request, Response};
use shared::{
    server_message::ErrorType,
    types::{ClientId, SessionToken},
};

use crate::AUTH_MANAGER;

#[allow(clippy::enum_variant_names)]
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Not authenticated")]
    AuthError,
    #[error("Bad request")]
    BadRequest,
    #[error("Challenge doesn't exist")]
    ChallengeNotFound,
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
    #[error("Crypto error: {0}")]
    CryptoError(#[from] ed25519_dalek::SignatureError),
    #[error("Randomness error: {0}")]
    GetrandomError(#[from] getrandom::Error),
    #[error("Serialization error: {0}")]
    SerdeError(#[from] serde_json::Error),
    #[error("Client not found: {0:?}")]
    ClientNotFound(ClientId),
    #[error("Client not connected: {0:?}")]
    ClientNotConnected(ClientId),
    #[error("Client already exists: {0:?}")]
    ClientAlreadyExists(ClientId),
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}

impl ResponseError for Error {
    fn status(&self) -> StatusCode {
        match self {
            Error::AuthError => StatusCode::UNAUTHORIZED,
            Error::BadRequest => StatusCode::BAD_REQUEST,
            Error::SerdeError(_) => StatusCode::BAD_REQUEST,
            Error::CryptoError(_) => StatusCode::BAD_REQUEST,
            Error::ChallengeNotFound => StatusCode::NOT_FOUND,
            Error::ClientNotConnected(_) => StatusCode::NOT_FOUND,
            Error::ClientNotFound(_) => StatusCode::NOT_FOUND,
            Error::IoError(_) => StatusCode::NOT_FOUND,
            Error::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::GetrandomError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::ClientAlreadyExists(_) => StatusCode::CONFLICT,
        }
    }

    fn as_response(&self) -> Response {
        let msg = match self {
            Error::AuthError => ErrorType::AuthError,
            Error::ChallengeNotFound => ErrorType::Retry,
            Error::DatabaseError(_) => ErrorType::ServerError("Database error".to_string()),
            Error::GetrandomError(_) => ErrorType::ServerError("Server error".to_string()),
            Error::BadRequest => ErrorType::BadRequest("Bad request".to_string()),
            Error::CryptoError(_) => ErrorType::BadRequest("Crypto error".to_string()),
            Error::SerdeError(_) => ErrorType::BadRequest("Serialization error".to_string()),
            Error::ClientAlreadyExists(_) => {
                ErrorType::BadRequest("Client already exists".to_string())
            }
            Error::ClientNotConnected(_) => ErrorType::DestinationUnreachable,
            Error::IoError(_) => ErrorType::DestinationUnreachable,
            Error::ClientNotFound(_) => ErrorType::DestinationUnreachable,
        };

        let body = Body::from_json(msg).expect("Failed to serialize response");
        Response::builder().status(self.status()).body(body)
    }
}

pub fn check_token_header(request: &Request) -> Result<ClientId, Error> {
    let token = request
        .headers()
        .get("Authorization")
        .ok_or(Error::AuthError)?
        .to_str()
        .map_err(|_| Error::AuthError)?;

    check_token(token).map_err(|_| Error::AuthError)
}

pub fn check_token(token: impl Into<String>) -> Result<ClientId, anyhow::Error> {
    let token: SessionToken = hex::decode(token.into())?
        .try_into()
        .map_err(|_| anyhow!("Invalid token length"))?;

    let client = AUTH_MANAGER
        .get()
        .unwrap()
        .get_session(token)
        .ok_or(anyhow!("Session doesn't exist"))?;

    Ok(client)
}
