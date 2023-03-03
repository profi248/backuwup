use serde::{Deserialize, Serialize};

pub enum ServerMessageWs {
    BackupMatched(BackupMatched),
    StorageChallengeRequest(StorageChallengeRequest),
    BackupTransferRequest(BackupTransferRequest),
}

#[derive(Serialize, Deserialize)]
pub struct BackupMatched {
    storage_available: u64,
    destination_id: crate::types::ClientId,
}

#[derive(Serialize, Deserialize)]
pub struct StorageChallengeRequest {
    // todo
}

#[derive(Serialize, Deserialize)]
pub struct BackupTransferRequest {
    // todo
}
