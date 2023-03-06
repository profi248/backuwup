use serde::{Deserialize, Serialize};

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
    pub server_challenge: [u8; 32],
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ClientLoginChallenge {
    server_challenge: [u8; 32],
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ClientLoginToken {
    token: [u8; 16],
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Error {
    Generic(String),
    BadRequest(String),
    ServerError(String),
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::BadRequest(e.to_string())
    }
}
