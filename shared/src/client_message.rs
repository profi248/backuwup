//! Messages sent by clients to server.

use serde::{Deserialize, Serialize};

use crate::types::{BlobHash, ChallengeResponse, ClientId, SessionToken, TransportSessionNonce};

/// The wrapper enum for all messages sent by clients.
#[derive(Serialize, Deserialize)]
pub enum ClientMessage {
    ClientRegistrationRequest(ClientRegistrationRequest),
    ClientRegistrationAuth(ClientRegistrationAuth),
    ClientLoginRequest(ClientLoginRequest),
    ClientLoginAuth(ClientLoginAuth),
    BackupRequest(BackupRequest),
}

/// The message for the first step of the client registration process.
#[derive(Serialize, Deserialize)]
pub struct ClientRegistrationRequest {
    pub client_id: ClientId,
}

/// The message for the third step of the client registration process.
#[derive(Serialize, Deserialize)]
pub struct ClientRegistrationAuth {
    pub client_id: ClientId,
    pub challenge_response: ChallengeResponse,
}

/// The message for the first step of the client login process.
#[derive(Serialize, Deserialize)]
pub struct ClientLoginRequest {
    pub client_id: ClientId,
}

/// The message for the third step of the client login process.
#[derive(Serialize, Deserialize)]
pub struct ClientLoginAuth {
    pub client_id: ClientId,
    pub challenge_response: ChallengeResponse,
}

/// The message for making a storage request.
#[derive(Serialize, Deserialize)]
pub struct BackupRequest {
    pub session_token: SessionToken,
    pub storage_required: u64,
}

/// The message sent by the client to establish a P2P connection.
#[derive(Serialize, Deserialize)]
pub struct BeginP2PConnectionRequest {
    pub session_token: SessionToken,
    pub destination_client_id: ClientId,
    pub session_nonce: TransportSessionNonce,
}

/// The message sent by the client to confirm an incoming P2P connection.
#[derive(Serialize, Deserialize)]
pub struct ConfirmP2PConnectionRequest {
    pub session_token: SessionToken,
    pub source_client_id: ClientId,
    pub destination_ip_address: String,
}

/// The message sent by the client to request info about a backup restore.
#[derive(Serialize, Deserialize)]
pub struct BackupRestoreRequest {
    pub session_token: SessionToken,
}

/// The message sent by the client to indicate that a backup has been completed.
#[derive(Serialize, Deserialize)]
pub struct BackupDone {
    pub session_token: SessionToken,
    pub snapshot_hash: BlobHash,
}
