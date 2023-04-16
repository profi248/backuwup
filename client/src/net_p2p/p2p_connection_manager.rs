use std::{
    fmt::{Debug, Formatter},
    sync::Arc,
    time::Duration,
};

use anyhow::anyhow;
use delay_map::HashMapDelay;
use shared::{
    p2p_message::RequestType,
    types::{ClientId, TransportSessionNonce},
};
use tokio::sync::Mutex;

const TRANSPORT_REQUEST_EXPIRY: Duration = Duration::from_secs(60);

pub struct P2PConnectionRequest {
    pub session_nonce: TransportSessionNonce,
    pub purpose: RequestType,
}

pub struct P2PConnectionManager {
    requests: Arc<Mutex<HashMapDelay<ClientId, P2PConnectionRequest>>>,
}

impl Debug for P2PConnectionManager {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "P2PConnectionManager")
    }
}

impl P2PConnectionManager {
    pub fn new() -> Self {
        Self {
            requests: Arc::new(Mutex::new(HashMapDelay::new(TRANSPORT_REQUEST_EXPIRY))),
        }
    }

    pub async fn add_request(
        &self,
        client_id: ClientId,
        purpose: RequestType,
    ) -> anyhow::Result<TransportSessionNonce> {
        let mut session_nonce: TransportSessionNonce = Default::default();
        getrandom::getrandom(&mut session_nonce)?;

        let mut requests = self.requests.lock().await;
        requests.insert(client_id, P2PConnectionRequest { session_nonce, purpose });

        Ok(session_nonce)
    }

    pub async fn finalize_request(&self, client_id: ClientId) -> anyhow::Result<P2PConnectionRequest> {
        let mut requests = self.requests.lock().await;
        match requests.remove(&client_id) {
            Some(request) => Ok(request),
            None => Err(anyhow!("unsolicited finalize request message")),
        }
    }
}
