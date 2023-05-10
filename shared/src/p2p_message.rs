//! Rypes of messages that are sent between peers.

use serde::{Deserialize, Serialize};

use crate::types::{MessageSignature, PackfileId, TransportSessionNonce};

/// The maximum size of a message sent between peers.
pub const MAX_ENCAPSULATED_BACKUP_CHUNK_SIZE: usize = 8 * 1_048_576; // 8 MiB

/// An entire encapsulated message with a signature.
#[derive(Serialize, Deserialize)]
pub struct EncapsulatedMsg {
    // bincode-encoded body
    pub body: Vec<u8>,
    // Ed25519 signature of bytes of body
    pub signature: MessageSignature,
}

/// A replay attack protection header for encapsulated messages.
#[derive(Serialize, Deserialize)]
pub struct Header {
    pub sequence_number: u64,
    pub session_nonce: TransportSessionNonce,
}

/// The body of a initialization request message containing the standard header and a request type.
#[derive(Serialize, Deserialize)]
pub struct EncapsulateRequestBody {
    pub header: Header,
    pub request: RequestType,
}

/// The request type for an initialization request message.
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[non_exhaustive]
pub enum RequestType {
    Transport,
    RestoreAll,
}

/// The body for a file transport message, containing the standard header, file info, and file data.
#[derive(Serialize, Deserialize)]
pub struct EncapsulatedFileBody {
    pub header: Header,
    pub file_info: FileInfo,
    pub data: Vec<u8>,
}

/// The type of file being transported, and its name.
#[derive(Serialize, Deserialize, Debug)]
pub enum FileInfo {
    Packfile(PackfileId),
    Index(u32),
}

/// The body for an acknowledgement message, containing the standard header and the sequence number.
#[derive(Serialize, Deserialize)]
pub struct AckBody {
    pub header: Header,
    pub acknowledged_sequence_number: u64,
}
