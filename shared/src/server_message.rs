//! Server messages sent to clients over HTTP.

use serde::{Deserialize, Serialize};

use crate::types::{BlobHash, ChallengeNonce, ClientId, SessionToken};

/// The wrapper enum for all messages sent by the server over HTTP.
#[derive(Serialize, Deserialize, Debug)]
pub enum ServerMessage {
    Ok,
    Error(ErrorType),
    ClientRegistrationChallenge(ClientRegistrationChallenge),
    ClientLoginChallenge(ClientLoginChallenge),
    ClientLoginToken(ClientLoginToken),
    BackupRestoreInfo(BackupRestoreInfo),
}

/// The message sent by the server to as the second step of the client registration process.
#[derive(Serialize, Deserialize, Debug)]
pub struct ClientRegistrationChallenge {
    pub server_challenge: ChallengeNonce,
}

/// The message sent by the server to as the second step of the client login process.
#[derive(Serialize, Deserialize, Debug)]
pub struct ClientLoginChallenge {
    pub server_challenge: ChallengeNonce,
}

/// The message sent by the server to as the last step of the client login process.
#[derive(Serialize, Deserialize, Debug)]
pub struct ClientLoginToken {
    pub token: SessionToken,
}

/// The message sent by the server containing information about the backup to restore.
#[derive(Serialize, Deserialize, Debug)]
pub struct BackupRestoreInfo {
    pub snapshot_hash: BlobHash,
    pub peers: Vec<ClientId>,
}

/// Error types for server responses.
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
