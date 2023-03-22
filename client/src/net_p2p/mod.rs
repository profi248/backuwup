use std::{
    fmt::{Debug, Formatter},
    sync::Arc,
    time::Duration,
};

use delay_map::HashMapDelay;
use getrandom::getrandom;
use shared::{
    p2p_message::MAX_ENCAPSULATED_BACKUP_CHUNK_SIZE,
    types::{ClientId, TransportSessionNonce},
};
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;

use crate::net_p2p::transport::BackupTransportManager;

const TRANSPORT_REQUEST_EXPIRY: Duration = Duration::from_secs(60);

pub mod receive;
pub mod transport;

pub struct TransportRequest {
    session_nonce: TransportSessionNonce,
}

pub struct TransportRequestManager {
    requests: Arc<Mutex<HashMapDelay<ClientId, TransportRequest>>>,
}

impl Debug for TransportRequestManager {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "TransportRequestManager")
    }
}

impl TransportRequestManager {
    pub fn new() -> Self {
        Self {
            requests: Arc::new(Mutex::new(HashMapDelay::new(TRANSPORT_REQUEST_EXPIRY))),
        }
    }

    pub async fn add_request(&self, client_id: ClientId) -> anyhow::Result<TransportSessionNonce> {
        let mut session_nonce: TransportSessionNonce = Default::default();
        getrandom(&mut session_nonce)?;

        let mut requests = self.requests.lock().await;
        requests.insert(client_id, TransportRequest { session_nonce });

        Ok(session_nonce)
    }

    pub async fn finalize_request(
        &self,
        client_id: ClientId,
        client_addr: String,
    ) -> anyhow::Result<Option<BackupTransportManager>> {
        let mut requests = self.requests.lock().await;
        match requests.remove(&client_id) {
            Some(request) => {
                Ok(Some(BackupTransportManager::new(client_addr, request.session_nonce).await?))
            }
            None => Ok(None),
        }
    }
}

fn get_ws_config() -> WebSocketConfig {
    WebSocketConfig {
        max_send_queue: None,
        max_message_size: Some(MAX_ENCAPSULATED_BACKUP_CHUNK_SIZE),
        max_frame_size: Some(MAX_ENCAPSULATED_BACKUP_CHUNK_SIZE),
        accept_unmasked_frames: false,
    }
}
