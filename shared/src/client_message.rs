use serde::{Deserialize, Serialize};

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
    pub client_id: crate::types::ClientId
}

#[derive(Serialize, Deserialize)]
pub struct ClientRegistrationAuth {
    pub client_id: crate::types::ClientId,
    pub challenge_response: [u8; 32]
}

#[derive(Serialize, Deserialize)]
pub struct ClientLoginRequest {}

#[derive(Serialize, Deserialize)]
pub struct ClientLoginAuth {
    client_id: crate::types::ClientId,
    challenge_response: [u8; 32],
}

#[derive(Serialize, Deserialize)]
pub struct BackupRequest {
    storage_required: u64,
    requester_id: crate::types::ClientId,
}
