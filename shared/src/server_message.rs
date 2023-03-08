use serde::{Deserialize, Serialize};

use crate::types::{ChallengeNonce, SessionToken};

#[derive(Serialize, Deserialize, Debug)]
pub enum ServerMessage {
    Ok,
    Error(Error),
    ClientRegistrationChallenge(ClientRegistrationChallenge),
    ClientLoginChallenge(ClientLoginChallenge),
    ClientLoginToken(ClientLoginToken),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ClientRegistrationChallenge {
    pub server_challenge: ChallengeNonce,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ClientLoginChallenge {
    pub server_challenge: ChallengeNonce,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ClientLoginToken {
    pub token: SessionToken,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Error {
    Failure(String),
    BadRequest(String),
    ServerError(String),
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::BadRequest(e.to_string())
    }
}
