//! WebSocket connections with clients.

use std::{
    collections::HashMap,
    fmt::{Debug, Formatter},
    sync::Arc,
};

use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use poem::{
    http::StatusCode,
    web::websocket::{Message, WebSocket, WebSocketStream},
    IntoResponse, Request,
};
use shared::{server_message_ws::ServerMessageWs, types::ClientId};
use tokio::sync::Mutex;

use crate::{handlers, handlers::check_token_header, CONNECTIONS};

/// Upgrade an incoming HTTP request to a WebSocket connection.
#[poem::handler]
pub async fn handler(ws: WebSocket, request: &Request) -> impl IntoResponse {
    let mut client_id = Default::default();
    let mut authorized = false;

    if let Ok(id) = check_token_header(request) {
        authorized = true;
        client_id = id;
    }

    ws.on_upgrade(move |mut socket| async move {
        if !authorized {
            println!("[ws] unauthorized WebSocket connection");
            return;
        }

        let (mut ws_send, mut ws_recv) = socket.split();

        // listen for incoming messages to handle if the channel is closed
        tokio::spawn(incoming_listener(ws_recv, client_id));

        // if the same client connects again, the old connection is removed from the map
        // automatically, but the old connection is still listening for incoming
        // messages, maybe send a message to the old connection to close it?
        CONNECTIONS.get().unwrap().new_connection(client_id, ws_send).await;

        println!("[ws] new connection: {}", hex::encode(client_id));
    })
    .with_status(if authorized { StatusCode::SWITCHING_PROTOCOLS } else { StatusCode::UNAUTHORIZED })
}

/// Listens for incoming messages on a WebSocket connection, to handle if the connection is closed.
pub async fn incoming_listener(mut ws_recv: SplitStream<WebSocketStream>, client_id: ClientId) {
    loop {
        let msg = ws_recv.next().await;

        // remove the sink if the connection is closed gracefully or if there is an error
        match msg {
            None | Some(Err(_) | Ok(Message::Close(_))) => {
                CONNECTIONS.get().unwrap().remove_connection(client_id).await;
                println!("[ws] connection dropped: {}", hex::encode(client_id));
                break;
            }
            _ => continue,
        }
    }
}

/// Keeps track of all active WebSocket connections to clients.
pub struct ClientConnections {
    connections: Arc<Mutex<HashMap<ClientId, SplitSink<WebSocketStream, Message>>>>,
}

impl ClientConnections {
    /// Creates a new instance of `ClientConnections`.
    pub fn new() -> Self {
        Self { connections: Arc::new(Mutex::new(HashMap::new())) }
    }

    /// Keeps track of a new connection.
    pub async fn new_connection(&self, client_id: ClientId, connection: SplitSink<WebSocketStream, Message>) {
        self.connections.lock().await.insert(client_id, connection);
    }

    /// Removes a connection.
    pub async fn remove_connection(&self, client_id: ClientId) {
        self.connections.lock().await.remove(&client_id);
    }

    /// Send a message to a connected client identified by its ID.
    pub async fn notify_client(
        &self,
        client_id: ClientId,
        message: ServerMessageWs,
    ) -> Result<(), handlers::Error> {
        let mut connections = self.connections.lock().await;
        let connection = connections
            .get_mut(&client_id)
            .ok_or(handlers::Error::ClientNotConnected(client_id))?;

        println!("[ws] notifying client {} with message {message:?}", hex::encode(client_id));
        connection
            .send(Message::Text(serde_json::to_string(&message)?))
            .await?;
        Ok(())
    }
}

impl Debug for ClientConnections {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "ClientConnections")
    }
}
