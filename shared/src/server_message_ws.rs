//! Messages sent to clients by server over WebSocket.

use serde::{Deserialize, Serialize};

use crate::types::{ClientId, TransportSessionNonce};

/// The wrapper enum for all messages sent by the server over WebSocket.
#[derive(Serialize, Deserialize, Debug)]
pub enum ServerMessageWs {
    Ping,
    BackupMatched(BackupMatched),
    IncomingP2PConnection(IncomingP2PConnection),
    FinalizeP2PConnection(FinalizeP2PConnection),
}

/// The message sent by the server to indicate that a backup has been matched.
#[derive(Serialize, Deserialize, Debug)]
pub struct BackupMatched {
    pub storage_available: u64,
    pub destination_id: ClientId,
}

/// The message sent by the server to to initiate a P2P connection.
#[derive(Serialize, Deserialize, Debug)]
pub struct IncomingP2PConnection {
    pub source_client_id: ClientId,
    pub session_nonce: TransportSessionNonce,
}

/// The message sent by the server to finalize a P2P connection.
#[derive(Serialize, Deserialize, Debug)]
pub struct FinalizeP2PConnection {
    pub destination_client_id: ClientId,
    pub destination_ip_address: String,
}
