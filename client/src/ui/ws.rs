//! WebSocket connection handler.

use futures_util::{stream::SplitSink, SinkExt, StreamExt};
use poem::{
    web::websocket::{Message, WebSocket, WebSocketStream},
    IntoResponse,
};
use tokio::sync::broadcast::{error::RecvError, Receiver};

use crate::{
    ui::{ws_dispatcher, ws_status_message::StatusMessage},
    UI,
};

/// Upgrade to a WebSocket connection and start sending log messages to the client.
#[poem::handler]
pub fn handler(ws: WebSocket) -> impl IntoResponse {
    // subscribe to the sender
    let mut log_receiver = UI.get().unwrap().subscribe();

    ws.on_upgrade(|mut socket| async move {
        // split the WebSocket into a separate Sink (for sending) and Stream (for receiving)
        let (ws_send, ws_recv) = socket.split();

        tokio::spawn(send_log_messages(ws_send, log_receiver));
        tokio::spawn(ws_dispatcher::dispatch_commands(ws_recv));
    })
}

/// Receives messages from the rest of the program and sends them to the WebSocket client.
async fn send_log_messages(
    mut ws_send: SplitSink<WebSocketStream, Message>,
    mut log_receiver: Receiver<StatusMessage>,
) {
    loop {
        let msg = match log_receiver.recv().await {
            // if the message is lagged (the log buffer is filled up),
            // ignore the error and try receiving again
            Err(RecvError::Lagged(_)) => continue,
            Err(e) => {
                println!("error: {e:?}");
                break;
            }
            Ok(msg) => msg,
        };

        match ws_send
            .send(Message::Text(serde_json::to_string(&msg).unwrap()))
            .await
        {
            // end the thread if sending to socket failed (it's likely closed)
            Err(_) => break,
            Ok(_) => continue,
        }
    }
}
