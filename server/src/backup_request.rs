use std::ops::Add;
use std::time::Duration;
use sum_queue::SumQueue;

const BACKUP_REQUEST_EXPIRY: Duration = Duration::from_secs(60 * 5);

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

pub struct Queue {
    queue: SumQueue<Request>,
}

impl Queue {
    pub fn new() -> Self {
        Self {
            queue: SumQueue::new(BACKUP_REQUEST_EXPIRY),
        }
    }

    pub fn push(&mut self, request: Request) {
        self.queue.push(request);
    }

    pub fn pop(&mut self) -> Option<Request> {
        self.queue.pop()
    }
}
