use serde::{Deserialize, Serialize};

use crate::types::{BlobHash, ChallengeNonce, ClientId, SessionToken};

#[derive(Serialize, Deserialize, Debug)]
pub enum ServerMessage {
    Ok,
    Error(ErrorType),
    ClientRegistrationChallenge(ClientRegistrationChallenge),
    ClientLoginChallenge(ClientLoginChallenge),
    ClientLoginToken(ClientLoginToken),
    BackupRestoreInfo(BackupRestoreInfo),
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
pub struct BackupRestoreInfo {
    pub snapshot_hash: BlobHash,
    pub peers: Vec<ClientId>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ErrorType {
    Unauthorized,
    ClientNotFound,
    DestinationUnreachable,
    NoBackups,
    Retry,
    BadRequest(String),
    ServerError(String),
    Failure(String),
}

impl From<serde_json::Error> for ErrorType {
    fn from(e: serde_json::Error) -> Self {
        ErrorType::BadRequest(e.to_string())
    }
}
