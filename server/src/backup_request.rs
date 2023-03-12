use std::{
    fmt::{Debug, Formatter},
    ops::Add,
    sync::{Arc, Mutex},
    time::Duration,
};

use shared::{
    client_message::BackupRequest,
    constants::{BACKUP_REQUEST_EXPIRY, MAX_BACKUP_STORAGE_REQUEST_SIZE},
    server_message_ws::ServerMessageWs,
};
use sum_queue::SumQueue;

use crate::{handlers, CONNECTIONS};

#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub struct Request {
    storage_required: u64,
    client_id: shared::types::ClientId,
}

impl From<BackupRequest> for Request {
    fn from(request: BackupRequest) -> Self {
        Self {
            storage_required: request.storage_required,
            client_id: request.requester_id,
        }
    }
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

impl Debug for Queue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "BackupRequest::Queue")
    }
}

impl Queue {
    pub fn new() -> Self {
        Self {
            queue: Arc::new(Mutex::new(SumQueue::new(Duration::from_secs(BACKUP_REQUEST_EXPIRY)))),
        }
    }

    /// The storage request fulfillment strategy is to put incoming requests in
    /// a queue with expiration. From there, they are removed and matched as
    /// new requests come in. As it's a queue, the requests that came first
    /// will be matched first. When processing a request, it is removed and
    /// the requested size is subtracted until it's completely fulfilled.
    pub async fn fulfill(&self, request: Request) -> Result<(bool, Vec<Request>), handlers::Error> {
        if request.storage_required == 0 {
            return Ok((true, Vec::new()));
        }

        if request.storage_required > MAX_BACKUP_STORAGE_REQUEST_SIZE {
            return Err(handlers::Error::BadRequest);
        }

        let mut storage_to_fulfill: i64 = request.storage_required as i64;
        let mut destinations = Vec::new();
        let mut fulfilled = true;

        while let Some(destination) = self.pop() {
            // don't match requests from the same client and discard them to avoid infiniteloops
            if destination.client_id == request.client_id {
                continue;
            }

            // notify client that its request has been fulfilled
            match CONNECTIONS
                .get()
                .unwrap()
                .notify_client(destination.client_id, ServerMessageWs::Ping)
                .await
            {
                Ok(_) => {
                    // add the fulfilled request to the list of destinations
                    destinations.push(destination.clone());
                    storage_to_fulfill -= destination.storage_required as i64;
                }
                Err(e) => {
                    // drop the request if the client is not connected and continue
                    println!(
                        "[backup request] failed to notify client {:?} of fulfilled request: {e}",
                        destination.client_id
                    );
                    continue;
                }
            }

            if storage_to_fulfill <= 0 {
                // partially fulfill the request of waiting destination
                // and put the remaining part back on the queue
                self.push(Request {
                    client_id: destination.client_id,
                    storage_required: ((destination.storage_required as i64) + storage_to_fulfill)
                        as u64,
                });

                break;
            }
        }

        // the incoming request is not entirely fulfilled (or not fulfilled at all),
        // so we'll put the remainder in the queue
        if storage_to_fulfill > 0 {
            self.push(Request {
                client_id: request.client_id,
                storage_required: storage_to_fulfill as u64,
            });

            fulfilled = false;
        }

        Ok((fulfilled, destinations))
    }

    pub fn debug_print(&self) {
        for request in self
            .queue
            .lock()
            .expect("Failed to lock backup request queue")
            .iter()
        {
            println!("[backup queue] {request:?}");
        }
    }

    fn push(&self, request: Request) {
        self.queue
            .lock()
            .expect("Failed to lock backup request queue")
            .push(request);
    }

    fn pop(&self) -> Option<Request> {
        self.queue
            .lock()
            .expect("Failed to lock backup request queue")
            .pop()
    }
}
