use std::{
    cmp::Ordering,
    fmt::{Debug, Formatter},
    ops::Add,
    sync::{Arc, Mutex},
    time::Duration,
};

use shared::{
    constants::{BACKUP_REQUEST_EXPIRY, MAX_BACKUP_STORAGE_REQUEST_SIZE},
    server_message_ws::{BackupMatched, ServerMessageWs},
    types::ClientId,
};
use sum_queue::SumQueue;

use crate::{handlers, CONNECTIONS};

#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub struct Request {
    pub storage_required: u64,
    pub client_id: ClientId,
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
    pub async fn fulfill(&self, request: Request) -> Result<(), handlers::Error> {
        if request.storage_required == 0 {
            return Ok(());
        }

        if request.storage_required > MAX_BACKUP_STORAGE_REQUEST_SIZE {
            return Err(handlers::Error::BadRequest);
        }

        let mut storage_to_fulfill: i64 = request.storage_required as i64;

        while let Some(destination) = self.pop() {
            // don't match requests from the same client and discard them to avoid infinite loops
            if destination.client_id == request.client_id {
                continue;
            }

            // notify a client that its enqueued request has been fulfilled
            let notify_result = CONNECTIONS
                .get()
                .unwrap()
                .notify_client(
                    destination.client_id,
                    ServerMessageWs::BackupMatched(BackupMatched {
                        storage_available: request.storage_required,
                        destination_id: request.client_id,
                    }),
                )
                .await;

            match notify_result {
                Ok(_) => {
                    // notify the requesting client that there's a match
                    CONNECTIONS
                        .get()
                        .unwrap()
                        .notify_client(
                            request.client_id,
                            ServerMessageWs::BackupMatched(BackupMatched {
                                storage_available: destination.storage_required,
                                destination_id: destination.client_id,
                            }),
                        )
                        .await?;

                    match storage_to_fulfill.cmp(&(destination.storage_required as i64)) {
                        // if destination's requested storage is greater then the remaining part of
                        // incoming request, we have successfully fulfilled the request and
                        // the remaining portion of destination's request will be put back on the queue
                        Ordering::Less => {
                            self.push(Request {
                                client_id: destination.client_id,
                                storage_required: destination.storage_required - request.storage_required,
                            });

                            break;
                        }
                        // if this destination hasn't fulfilled our incoming request completely,
                        // we will subtract from the storage that still need fulfilling,
                        // and continue with other requests
                        Ordering::Greater => {
                            storage_to_fulfill -= destination.storage_required as i64;

                            continue;
                        }
                        // if requested amounts are equal, both clients have been notified,
                        // requests have been discarded and we are done
                        Ordering::Equal => break,
                    }
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
        }

        // the incoming request is not entirely fulfilled (or not fulfilled at all),
        // so we'll put the remainder in the queue
        if storage_to_fulfill > 0 {
            self.push(Request {
                client_id: request.client_id,
                storage_required: storage_to_fulfill as u64,
            });
        }

        Ok(())
    }

    pub fn debug_print(&self) {
        for request in self.queue.lock().expect("Failed to lock backup request queue").iter() {
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
        self.queue.lock().expect("Failed to lock backup request queue").pop()
    }
}
