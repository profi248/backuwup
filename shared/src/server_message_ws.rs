use serde::{Deserialize, Serialize};

use crate::types::{ClientId, TransportSessionNonce};

#[derive(Serialize, Deserialize, Debug)]
pub enum ServerMessageWs {
    Ping,
    BackupMatched(BackupMatched),
    IncomingP2PConnection(IncomingP2PConnection),
    FinalizeP2PConnection(FinalizeP2PConnection),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BackupMatched {
    pub storage_available: u64,
    pub destination_id: ClientId,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IncomingP2PConnection {
    pub source_client_id: ClientId,
    pub session_nonce: TransportSessionNonce,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FinalizeP2PConnection {
    pub destination_client_id: ClientId,
    pub destination_ip_address: String,
}
