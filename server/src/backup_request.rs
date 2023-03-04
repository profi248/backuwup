use std::ops::Add;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use anyhow::bail;
use sum_queue::SumQueue;
use shared::constants::{BACKUP_REQUEST_EXPIRY, MAX_BACKUP_STORAGE_REQUEST_SIZE};
use crate::CONNECTIONS;

#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub struct Request {
    storage_required: u64,
    client_id: shared::types::ClientId,
}

// sum will be the total requested space
impl Add for Request {
    type Output = u64;

    fn add(self, rhs: Self) -> Self::Output {
        self.storage_required + rhs.storage_required
    }
}

#[derive(Clone)]
pub struct Queue {
    queue: Arc<Mutex<SumQueue<Request>>>,
}

impl Queue {
    pub fn new() -> Self {
        Self {
            queue: Arc::new(Mutex::new(SumQueue::new(Duration::from_secs(BACKUP_REQUEST_EXPIRY))))
        }
    }

    /// The storage request fulfill strategy is to put incoming requests in a queue with expiration,
    /// from where they are removed and matched as new requests come in. As it's a queue, the
    /// requests that came first will be matched first. When processing a request,
    /// they are removed and the requested size is subtracted until it's completely fulfilled.
    pub async fn fulfill(&mut self, request: &Request) -> anyhow::Result<(bool, Vec<Request>)> {
        if request.storage_required == 0 {
            return Ok((true, Vec::new()));
        }

        if request.storage_required > MAX_BACKUP_STORAGE_REQUEST_SIZE {
            bail!("Requested storage size exceeds the maximum allowed size");
        }

        let mut storage_to_fulfill: i64 = request.storage_required as i64;
        let mut destinations = Vec::new();
        let mut fulfilled = true;

        while let Some(destination) = self.pop() {
            storage_to_fulfill -= destination.storage_required as i64;
            destinations.push(destination.clone());

            // idea: either use message channels to deliver the message via websockets
            // (store the websocket channel in the queue, or via a separate handler?)
            // or maybe store them elsewhere until it's fetched over http

            // todo notify the client that the request was fulfilled
            CONNECTIONS.get().expect("OnceCell failed")
                .notify_client(destination.client_id, "Backup request fulfilled".to_string()).await?;

            if storage_to_fulfill <= 0 {
                // partially fulfill the request of waiting destination
                // and put the remaining part back on the queue
                self.push(Request {
                    client_id: destination.client_id,
                    storage_required: ((destination.storage_required as i64) + storage_to_fulfill) as u64
                });

                break
            }
        }

        // the incoming request is not entirely fulfilled (or not fulfilled at all),
        // so we'll put the remainder in the queue
        if storage_to_fulfill > 0 {
            self.push(Request {
               client_id: request.client_id,
               storage_required: storage_to_fulfill as u64
            });

            fulfilled = false;
        }

        Ok((fulfilled, destinations))
    }

    fn push(&mut self, request: Request) {
        self.queue.lock()
            .expect("Failed to lock backup request queue")
            .push(request);
    }

    fn pop(&mut self) -> Option<Request> {
        self.queue.lock()
            .expect("Failed to lock backup request queue")
            .pop()
    }
}
