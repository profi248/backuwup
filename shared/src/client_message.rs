use serde::{Deserialize, Serialize};

use crate::types::{ChallengeResponse, ClientId, SessionToken, TransportSessionNonce};

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
    pub client_id: crate::types::ClientId,
}

#[derive(Serialize, Deserialize)]
pub struct ClientRegistrationAuth {
    pub client_id: crate::types::ClientId,
    pub challenge_response: ChallengeResponse,
}

#[derive(Serialize, Deserialize)]
pub struct ClientLoginRequest {
    pub client_id: crate::types::ClientId,
}

#[derive(Serialize, Deserialize)]
pub struct ClientLoginAuth {
    pub client_id: crate::types::ClientId,
    pub challenge_response: ChallengeResponse,
}

#[derive(Serialize, Deserialize)]
pub struct BackupRequest {
    pub client_token: crate::types::ClientToken,
    pub storage_required: u64,
    pub requester_id: crate::types::ClientId,
}

#[derive(Serialize, Deserialize)]
pub struct BeginTransportRequest {
    pub session_token: SessionToken,
    pub destination_client_id: ClientId,
    pub session_nonce: TransportSessionNonce,
}

#[derive(Serialize, Deserialize)]
pub struct ConfirmTransportRequest {
    pub session_token: SessionToken,
    pub source_client_id: ClientId,
    pub destination_ip_address: String,
}
