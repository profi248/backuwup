use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum ServerMessage {
    Ok,
    Error(Error),
    ClientRegistrationChallenge(ClientRegistrationChallenge),
    ClientLoginChallenge(ClientLoginChallenge),
    ClientLoginToken(ClientLoginToken),
}

#[derive(Serialize, Deserialize)]
pub struct ClientRegistrationChallenge {
    pub server_challenge: [u8; 32]
}

#[derive(Serialize, Deserialize)]
pub struct ClientLoginChallenge {
    server_challenge: [u8; 32]
}

#[derive(Serialize, Deserialize)]
pub struct ClientLoginToken {
    token: [u8; 16],
}

#[derive(Serialize, Deserialize)]
pub enum Error {
    BadRequest(String),
    ServerError(String),
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::BadRequest(e.to_string())
    }
}
