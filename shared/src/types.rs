//! Defines types used across server and client.

/// The type for client ID/the client's public key.
pub type ClientId = [u8; 32];

/// The type for nonce sent by the server as a part of the authentication challenge.
pub type ChallengeNonce = [u8; 16];

/// The type for the response to the authentication challenge, a signed nonce.
pub type ChallengeResponse = Vec<u8>; // this should be of fixed size 64, but serde doesn't like it

/// The type for the session token, used to authenticate the client in subsequent requests.
pub type SessionToken = [u8; 16];

/// The type for the nonce used to secure the transport session.
pub type TransportSessionNonce = [u8; 16];

/// The type for the signature of the message sent by the client in P2P communication.
pub type MessageSignature = Vec<u8>; // this should be of fixed size 64, but serde doesn't like it

/// The length of `ChallengeResponse`.
pub const CHALLENGE_RESPONSE_LENGTH: usize = 64;

/// The length of `MessageSignature`.
pub const MESSAGE_SIGNATURE_LENGTH: usize = 64;

/// The size of a blob encryption nonce in bytes.
pub const BLOB_NONCE_SIZE: usize = 12;

/// The type for the hash of a blob.
pub type BlobHash = [u8; 32];

/// The type for the hash of a packfile.
pub type PackfileId = [u8; 12];

/// The type for the encryption nonce of a blob.
pub type BlobNonce = [u8; BLOB_NONCE_SIZE];
