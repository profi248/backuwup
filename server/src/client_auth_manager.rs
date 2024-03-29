//! Keeps track the challenge-response authentication process and active sessions.

use std::{
    fmt::{Debug, Formatter},
    sync::{Arc, Mutex},
    time::Duration,
};

use delay_map::HashMapDelay;
use ed25519_dalek::{PublicKey, Signature};
use getrandom::getrandom;
use shared::types::{ChallengeNonce, ChallengeResponse, ClientId, SessionToken};

use crate::handlers;

/// Specifies how long a challenge nonce is valid for.
const CHALLENGE_EXPIRATION: Duration = Duration::from_secs(30);

/// Specifies how long a session token is valid for, at maximum.
const SESSION_EXPIRATION: Duration = Duration::from_secs(24 * 3600);

/// Keeps track of client authentication challenges and active sessions.
pub struct ClientAuthManager {
    data: Arc<Mutex<ClientAuthManagerInner>>,
}

impl Debug for ClientAuthManager {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "ClientAuthManager")
    }
}

struct ClientAuthManagerInner {
    challenges: HashMapDelay<ClientId, ChallengeNonce>,
    sessions: HashMapDelay<SessionToken, ClientId>,
}

impl ClientAuthManager {
    pub fn new() -> Self {
        let inner = ClientAuthManagerInner {
            challenges: HashMapDelay::new(CHALLENGE_EXPIRATION),
            sessions: HashMapDelay::new(SESSION_EXPIRATION),
        };

        Self { data: Arc::new(Mutex::new(inner)) }
    }

    /// Starts a new challenge, given client, generating a random nonce.
    pub fn challenge_begin(&self, client_id: ClientId) -> Result<ChallengeNonce, handlers::Error> {
        let mut nonce: ChallengeNonce = Default::default();
        getrandom(&mut nonce)?;

        let mut data = self.data.lock().expect("Lock failed");

        // a client can have one challenge nonce at once, if it already exists it will be replaced
        data.challenges.insert(client_id, nonce);

        Ok(nonce)
    }

    /// Verifies a response to challenge that has been started before.
    pub fn challenge_verify(
        &self,
        client_id: ClientId,
        response: &ChallengeResponse,
    ) -> Result<(), handlers::Error> {
        let mut data = self.data.lock().expect("Lock failed");
        let nonce = data.challenges.get(&client_id);

        if nonce.is_none() {
            return Err(handlers::Error::ChallengeNotFound);
        };

        let client_pubkey = PublicKey::from_bytes(&client_id)?;
        let signature = Signature::from_bytes(response)?;

        // use the newer, stricter method to verify whether the signature is valid
        client_pubkey.verify_strict(nonce.unwrap(), &signature)?;

        data.challenges.remove(&client_id);
        Ok(())
    }

    /// Generates a new random session token and associates it with the given client.
    pub fn session_start(&self, client_id: ClientId) -> Result<SessionToken, handlers::Error> {
        let mut token: SessionToken = Default::default();
        getrandom(&mut token)?;

        let mut data = self.data.lock().expect("Lock failed");

        data.sessions.insert(token, client_id);

        Ok(token)
    }

    /// Returns the client associated with the given session token.
    pub fn get_session(&self, token: SessionToken) -> Option<ClientId> {
        let data = self.data.lock().unwrap();

        data.sessions.get(&token).copied()
    }
}
