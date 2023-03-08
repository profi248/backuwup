use serde::{Deserialize, Serialize};

use crate::types::ChallengeResponse;

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
pub struct ClientLoginRequest {}

#[derive(Serialize, Deserialize)]
pub struct ClientLoginAuth {
    client_id: crate::types::ClientId,
    challenge_response: ChallengeResponse,
}

#[derive(Serialize, Deserialize)]
pub struct BackupRequest {
    pub client_token: crate::types::ClientToken,
    pub storage_required: u64,
    pub requester_id: crate::types::ClientId,
}
