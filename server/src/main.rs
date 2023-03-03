#![deny(unused_must_use, deprecated)]
#![warn(clippy::pedantic)]

mod backup_request;
mod db;
mod ws;
mod handlers;

use crate::db::Database;
use poem::listener::{Listener, RustlsCertificate, RustlsConfig};
use poem::{listener::TcpListener, EndpointExt, Route, Server};

#[tokio::main]
async fn main() {
    let bind_ip = dotenv::var("BIND_IP").expect("BIND_IP environment variable not set or invalid");
    let cert = dotenv::var("TLS_CERT").expect("TLS_CERT environment variable not set or invalid");
    let key = dotenv::var("TLS_KEY").expect("TLS_KEY environment variable not set or invalid");

    let db = Database::init().await;
    let backup_request_queue = backup_request::Queue::new();

    let app = Route::new()
        .at("/register", handlers::register::register.data(db.clone()))
        .at("/ws", ws::handler.data(db));

    let config = RustlsConfig::new().fallback(RustlsCertificate::new().cert(cert).key(key));

    let listener = TcpListener::bind(bind_ip).rustls(config);
    let server = Server::new(listener);
    server.run(app).await.unwrap();
}
