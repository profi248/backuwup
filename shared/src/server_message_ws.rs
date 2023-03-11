use std::net::IpAddr;

use serde::{Deserialize, Serialize};

use crate::types::{ClientId, TransportSessionNonce};

#[derive(Serialize, Deserialize, Debug)]
pub enum ServerMessageWs {
    Ping,
    BackupMatched(BackupMatched),
    IncomingTransportRequest(IncomingTransportRequest),
    FinalizeTransportRequest(FinalizeTransportRequest),
    StorageChallengeRequest(StorageChallengeRequest),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BackupMatched {
    storage_available: u64,
    destination_id: crate::types::ClientId,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IncomingTransportRequest {
    source_client_id: ClientId,
    session_nonce: TransportSessionNonce,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FinalizeTransportRequest {
    destination_client_id: ClientId,
    destination_ip_address: IpAddr,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StorageChallengeRequest {
    // todo
}
