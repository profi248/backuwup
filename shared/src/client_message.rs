use serde::{Deserialize, Serialize};

use crate::types::{BlobHash, ChallengeResponse, ClientId, SessionToken, TransportSessionNonce};

#[derive(Serialize, Deserialize)]
pub enum ClientMessage {
    ClientRegistrationRequest(ClientRegistrationRequest),
    ClientRegistrationAuth(ClientRegistrationAuth),
    ClientLoginRequest(ClientLoginRequest),
    ClientLoginAuth(ClientLoginAuth),
    BackupRequest(BackupRequest),
}

#[derive(Serialize, Deserialize)]
pub struct ClientRegistrationRequest {
    pub client_id: ClientId,
}

#[derive(Serialize, Deserialize)]
pub struct ClientRegistrationAuth {
    pub client_id: ClientId,
    pub challenge_response: ChallengeResponse,
}

#[derive(Serialize, Deserialize)]
pub struct ClientLoginRequest {
    pub client_id: ClientId,
}

#[derive(Serialize, Deserialize)]
pub struct ClientLoginAuth {
    pub client_id: ClientId,
    pub challenge_response: ChallengeResponse,
}

#[derive(Serialize, Deserialize)]
pub struct BackupRequest {
    pub session_token: SessionToken,
    pub storage_required: u64,
}

#[derive(Serialize, Deserialize)]
pub struct BeginP2PConnectionRequest {
    pub session_token: SessionToken,
    pub destination_client_id: ClientId,
    pub session_nonce: TransportSessionNonce,
}

#[derive(Serialize, Deserialize)]
pub struct ConfirmP2PConnectionRequest {
    pub session_token: SessionToken,
    pub source_client_id: ClientId,
    pub destination_ip_address: String,
}

#[derive(Serialize, Deserialize)]
pub struct BackupRestoreRequest {
    pub session_token: SessionToken,
}

#[derive(Serialize, Deserialize)]
pub struct BackupDone {
    pub session_token: SessionToken,
    pub snapshot_hash: BlobHash,
}
