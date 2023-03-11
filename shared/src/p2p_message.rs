use serde::{Deserialize, Serialize};

use crate::types::{MessageSignature, TransportSessionNonce};

#[derive(Serialize, Deserialize)]
pub struct EncapsulatedBackupChunk {
    body: EncapsulatedBackupChunkBody,
    signature: MessageSignature,
}

#[derive(Serialize, Deserialize)]
pub struct EncapsulatedBackupChunkBody {
    sequence_number: u64,
    session_nonce: TransportSessionNonce,
    data: Vec<u8>,
}
