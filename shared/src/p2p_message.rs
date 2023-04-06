use serde::{Deserialize, Serialize};

use crate::types::{MessageSignature, PackfileId, TransportSessionNonce};

pub const MAX_ENCAPSULATED_BACKUP_CHUNK_SIZE: usize = 8 * 1_048_576; // 8 MiB

#[derive(Serialize, Deserialize)]
pub struct EncapsulatedPackfile {
    // bincode-encoded BackupChunkBody
    pub body: Vec<u8>,
    // Ed25519 signature of bytes of body
    pub signature: MessageSignature,
}

#[derive(Serialize, Deserialize)]
pub struct PackfileBody {
    pub sequence_number: u64,
    pub session_nonce: TransportSessionNonce,
    pub id: PackfileId,
    pub data: Vec<u8>,
}
