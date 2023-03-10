use std::{
    fmt::{Debug, Formatter},
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::bail;
use delay_map::HashMapDelay;
use ed25519_dalek::{PublicKey, Signature};
use getrandom::getrandom;
use shared::types::{ChallengeNonce, ChallengeResponse, ClientId, SessionToken};

const CHALLENGE_EXPIRATION: Duration = Duration::from_secs(30);
const SESSION_EXPIRATION: Duration = Duration::from_secs(24 * 3600);

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

        Self {
            data: Arc::new(Mutex::new(inner)),
        }
    }

    pub fn challenge_begin(&self, client_id: ClientId) -> anyhow::Result<ChallengeNonce> {
        let mut nonce: ChallengeNonce = Default::default();
        getrandom(&mut nonce)?;

        let mut data = self.data.lock().expect("Lock failed");

        // a client can have one challenge nonce at once, if it already exists it will be replaced
        data.challenges.insert(client_id, nonce);

        Ok(nonce)
    }

    pub fn challenge_verify(
        &self,
        client_id: ClientId,
        response: ChallengeResponse,
    ) -> anyhow::Result<()> {
        let mut data = self.data.lock().expect("Lock failed");
        let nonce = data.challenges.get(&client_id);

        if nonce.is_none() {
            bail!("Challenge for this client is expired or wasn't created");
        };

        let client_pubkey = PublicKey::from_bytes(&client_id)?;
        let signature = Signature::from_bytes(&response)?;

        // use the newer, stricter method to verify whether the signature is valid
        client_pubkey.verify_strict(nonce.unwrap(), &signature)?;

        data.challenges.remove(&client_id);
        Ok(())
    }

    pub fn session_start(&self, client_id: ClientId) -> anyhow::Result<SessionToken> {
        let mut token: SessionToken = Default::default();
        getrandom(&mut token)?;

        let mut data = self.data.lock().expect("Lock failed");

        data.sessions.insert(token, client_id);

        Ok(token)
    }

    pub fn session_clear(&self, token: SessionToken) -> anyhow::Result<()> {
        self.data.lock().expect("Lock failed").sessions.remove(&token);

        Ok(())
    }
}