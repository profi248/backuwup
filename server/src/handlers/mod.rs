pub mod backup;
pub mod backup_request;
pub mod login;
pub mod p2p_connection_request;
pub mod register;

use anyhow::anyhow;
use poem::{error::ResponseError, http::StatusCode, Body, Request, Response};
use shared::{
    server_message::{ErrorType, ServerMessage},
    types::{ClientId, SessionToken},
};

use crate::AUTH_MANAGER;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Not authorized")]
    Unauthorized,
    #[error("Bad request")]
    BadRequest,
    #[error("Challenge doesn't exist")]
    ChallengeNotFound,
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Invalid data found in database")]
    DatabaseTypeMismatch,
    #[error("No backups available")]
    NoBackupsAvailable,
    #[error("Crypto error: {0}")]
    Crypto(#[from] ed25519_dalek::SignatureError),
    #[error("Randomness error: {0}")]
    Rng(#[from] getrandom::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Client not found: {0:?}")]
    ClientNotFound(ClientId),
    #[error("Client not connected: {0:?}")]
    ClientNotConnected(ClientId),
    #[error("Client already exists: {0:?}")]
    ClientExists(ClientId),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl ResponseError for Error {
    fn status(&self) -> StatusCode {
        match self {
            Error::Unauthorized => StatusCode::UNAUTHORIZED,
            Error::BadRequest => StatusCode::BAD_REQUEST,
            Error::Serialization(_) => StatusCode::BAD_REQUEST,
            Error::Crypto(_) => StatusCode::BAD_REQUEST,
            Error::ChallengeNotFound => StatusCode::NOT_FOUND,
            Error::ClientNotConnected(_) => StatusCode::NOT_FOUND,
            Error::ClientNotFound(_) => StatusCode::NOT_FOUND,
            Error::Io(_) => StatusCode::NOT_FOUND,
            Error::NoBackupsAvailable => StatusCode::NOT_FOUND,
            Error::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::Rng(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::DatabaseTypeMismatch => StatusCode::INTERNAL_SERVER_ERROR,
            Error::ClientExists(_) => StatusCode::CONFLICT,
        }
    }

    fn as_response(&self) -> Response {
        let msg = match self {
            Error::Unauthorized => ErrorType::Unauthorized,
            Error::ChallengeNotFound => ErrorType::Retry,
            Error::Database(_) => ErrorType::ServerError("Database error".to_string()),
            Error::Rng(_) => ErrorType::ServerError("Server error".to_string()),
            Error::DatabaseTypeMismatch => ErrorType::ServerError("Encountered invalid data".to_string()),
            Error::BadRequest => ErrorType::BadRequest("Bad request".to_string()),
            Error::Crypto(_) => ErrorType::BadRequest("Crypto error".to_string()),
            Error::Serialization(_) => ErrorType::BadRequest("Serialization error".to_string()),
            Error::ClientExists(_) => ErrorType::BadRequest("Client already exists".to_string()),
            Error::ClientNotConnected(_) => ErrorType::DestinationUnreachable,
            Error::Io(_) => ErrorType::DestinationUnreachable,
            Error::ClientNotFound(_) => ErrorType::DestinationUnreachable,
            Error::NoBackupsAvailable => ErrorType::NoBackups,
        };

        println!("[err] sending error response to client: {msg:?}");
        let body = Body::from_json(ServerMessage::Error(msg)).expect("Failed to serialize response");
        Response::builder().status(self.status()).body(body)
    }
}

pub fn check_token_header(request: &Request) -> Result<ClientId, Error> {
    let token = request
        .headers()
        .get("Authorization")
        .ok_or(Error::Unauthorized)?
        .to_str()
        .map_err(|_| Error::Unauthorized)?;

    check_token(token).map_err(|_| Error::Unauthorized)
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
