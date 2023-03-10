use tokio::sync::broadcast::{Receiver, Sender};

#[derive(Debug)]
pub struct Logger {
    sender: Sender<String>,
}

impl Logger {
    pub fn new(sender: Sender<String>) -> Self {
        Self { sender }
    }

    pub fn send(&self, msg: impl Into<String> + Clone) {
        // ignore sending errors because they are not very meaningful
        self.sender.send(msg.clone().into()).ok();
        println!("[log] {}", msg.into());
    }

    pub fn subscribe(&self) -> Receiver<String> {
        self.sender.subscribe()
    }
}
