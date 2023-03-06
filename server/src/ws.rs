use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::db::Database;

use futures_util::{SinkExt, StreamExt};
use futures_util::stream::{SplitSink, SplitStream};
use poem::web::websocket::{Message, WebSocket, WebSocketStream};
use poem::{web::Data, IntoResponse};
use shared::server_message_ws::ServerMessageWs;
use shared::types::ClientId;
use crate::CONNECTIONS;

#[poem::handler]
pub async fn handler(ws: WebSocket, Data(db): Data<&Database>) -> impl IntoResponse {
    ws.on_upgrade(|mut socket| async {
        // todo authenticate here (probably using the token from login)
        let client_id = [0; 32];

        let (mut ws_send, mut ws_recv) = socket.split();

        // listen for incoming messages to handle if the channel is closed
        tokio::spawn(incoming_listener(ws_recv, client_id));

        // if the same client connects again, the old connection is removed from the map
        // automatically, but the old connection is still listening for incoming messages
        // todo maybe send a message to the old connection to close it?
        CONNECTIONS.get().expect("OnceCell failed")
            .new_connection(client_id, ws_send).await;

        println!("[ws] new connection: {client_id:?}");
    })
}

pub async fn incoming_listener(mut ws_recv: SplitStream<WebSocketStream>, client_id: ClientId) {
    loop {
        let msg = ws_recv.next().await;

        // remove the sink if the connection is closed gracefully or if there is an error
        match msg {
            None | Some(Err(_) | Ok(Message::Close(_))) => {
                CONNECTIONS.get().expect("OnceCell failed")
                    .remove_connection(client_id).await;

                println!("[ws] connection dropped: {client_id:?}");
                break;
            },
            _ => continue
        }
    };
}

pub struct ClientConnections {
    connections: Arc<Mutex<HashMap<ClientId, SplitSink<WebSocketStream, Message>>>>
}

impl ClientConnections {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(Mutex::new(HashMap::new()))
        }
    }

    pub async fn new_connection(&self, client_id: ClientId, connection: SplitSink<WebSocketStream, Message>) {
        self.connections.lock().await.insert(client_id, connection);
    }

    pub async fn remove_connection(&self, client_id: ClientId) {
        self.connections.lock().await.remove(&client_id);
    }

    pub async fn notify_client(&self, client_id: ClientId, message: ServerMessageWs) -> anyhow::Result<()> {
        let mut connections = self.connections.lock().await;
        let connection = connections.get_mut(&client_id).ok_or(anyhow::anyhow!("Client connection not found"))?;

        println!("[ws] notifying client {client_id:?} with message {message:?}");
        connection.send(Message::Text(serde_json::to_string(&message)?)).await?;
        Ok(())
    }
}

impl Debug for ClientConnections {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "ClientConnections")
    }
}
