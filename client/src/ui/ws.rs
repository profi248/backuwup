use futures_util::{stream::{SplitSink, SplitStream}, SinkExt, StreamExt};
use poem::{
    web::{
        websocket::{Message, WebSocket, WebSocketStream},
        Data,
    },
    IntoResponse,
};
use tokio::sync::broadcast::{error::RecvError, Receiver, Sender};

#[poem::handler]
pub fn handler(
    ws: WebSocket,
    Data(log_sender): Data<&Sender<String>>
) -> impl IntoResponse {
    // subscribe to the sender
    let mut log_receiver = log_sender.subscribe();

    ws.on_upgrade(|mut socket| async move {
        // split the WebSocket into a separate Sink (for sending) and Stream (for receiving)
        let (ws_send, ws_recv) = socket.split();

        tokio::spawn(send_log_messages(ws_send, log_receiver));
        tokio::spawn(dispatch_commands(ws_recv));
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
            Err(RecvError::Lagged(_)) => continue,
            Err(e) => { println!("error: {e:?}"); break },
            Ok(msg) => msg,
        };

        // todo: handle errors here properly
        ws_send.send(Message::Text(msg)).await.unwrap();
    }
}

async fn dispatch_commands(mut ws_recv: SplitStream<WebSocketStream>) {
    loop {
        let msg = match ws_recv.next().await {
            // for when the socket has closed
            None => break,
            Some(Ok(msg)) => msg,
            Some(Err(e)) => { println!("error: {e:?}"); break }
        };

        let msg = match msg {
            Message::Text(s) => s,
            _ => continue
        };

        // todo: implement a message dispatcher
        println!("message from client: {msg}");
    }
}
