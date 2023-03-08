pub type ClientId = [u8; 32];
pub type ClientToken = [u8; 16];
pub type ChallengeNonce = [u8; 16];
pub type ChallengeResponse = Vec<u8>; // this should be of fixed size 64, but serde doesn't like it
pub type SessionToken = [u8; 16];

pub const CHALLENGE_RESPONSE_LENGTH: usize = 64;
