use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum ServerMessageWs {
    Ping,
    BackupMatched(BackupMatched),
    StorageChallengeRequest(StorageChallengeRequest),
    BackupTransferRequest(BackupTransferRequest),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BackupMatched {
    storage_available: u64,
    destination_id: crate::types::ClientId,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StorageChallengeRequest {
    // todo
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BackupTransferRequest {
    // todo
}
