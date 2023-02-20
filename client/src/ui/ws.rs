
use futures_util::{Sink, SinkExt, StreamExt};
use futures_util::stream::SplitSink;


use poem::web::websocket::{Message, WebSocket, WebSocketStream};
use poem::web::Data;
use poem::{EndpointExt, IntoResponse};


use tokio::sync::broadcast::error::RecvError::Lagged;
use tokio::sync::broadcast::{Receiver, Sender};

#[poem::handler]
pub fn handle(ws: WebSocket, Data(log_sender): Data<&Sender<String>>) -> impl IntoResponse {
    // subscribe to the sender
    let mut log_receiver = log_sender.subscribe();

    ws.on_upgrade(|mut socket| async move {
        // split the WebSocket into a separate Sink (for sending) and Stream (for receiving)
        let (ws_send, _ws_recv) = socket.split();

        tokio::spawn(send_log_messages(ws_send, log_receiver));
    })
}

async fn send_log_messages(
    mut ws_send: SplitSink<WebSocketStream, Message>,
    mut log_receiver: Receiver<String>,
) {
    loop {
        let msg = match log_receiver.recv().await {
            // if the message is lagged (the log buffer is filled up),
            // ignore the error and try receiving again
            Err(Lagged(_)) => continue,
            Err(e) => panic!("{e:?}"),
            Ok(msg) => msg,
        };

        ws_send.send(Message::Text(msg)).await.unwrap();
    }
}
