#![deny(unused_must_use, deprecated)]
#![warn(clippy::pedantic)]

mod backup_request;
mod db;
mod handlers;
mod ws;

use crate::db::Database;
use crate::handlers::register::{register_begin, register_complete};
use delay_map::HashMapDelay;
use poem::{
    listener::{Listener, RustlsCertificate, RustlsConfig, TcpListener},
    EndpointExt, Route, Server,
};
use shared::types::ClientId;
use std::{sync::Arc, sync::Mutex, time::Duration};

type Challenges = Arc<Mutex<HashMapDelay<ClientId, [u8; 32]>>>;

#[tokio::main]
async fn main() {
    let bind_ip = dotenv::var("BIND_IP").expect("BIND_IP environment variable not set or invalid");
    let cert = dotenv::var("TLS_CERT").expect("TLS_CERT environment variable not set or invalid");
    let key = dotenv::var("TLS_KEY").expect("TLS_KEY environment variable not set or invalid");

    let db = Database::init().await;
    let backup_request_queue = backup_request::Queue::new();
    let challenge_tokens: Challenges =
        Arc::new(Mutex::new(HashMapDelay::new(Duration::from_secs(30))));

    let app = Route::new()
        .at(
            "/register/begin",
            register_begin.data(challenge_tokens.clone()),
        )
        .at(
            "/register/complete",
            register_complete.data(challenge_tokens.clone()),
        )
        .at("/ws", ws::handler.data(db));

    let config = RustlsConfig::new().fallback(RustlsCertificate::new().cert(cert).key(key));

    let listener = TcpListener::bind(bind_ip).rustls(config);
    let server = Server::new(listener);
    server.run(app).await.unwrap();
}
