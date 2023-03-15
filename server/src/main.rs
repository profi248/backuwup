#![deny(unused_must_use, deprecated)]
#![warn(clippy::pedantic)]
#![allow(clippy::single_match_else)]

mod backup_request;
mod client_auth_manager;
mod db;
mod handlers;
mod ws;

use poem::{
    listener::{RustlsCertificate, RustlsConfig, TcpListener},
    EndpointExt, Route, Server,
};
use tokio::sync::OnceCell;

use crate::{
    backup_request::Queue,
    client_auth_manager::ClientAuthManager,
    db::Database,
    handlers::{
        backup_request::make_backup_request,
        login::{login_begin, login_complete},
        register::{register_begin, register_complete},
        transport_request::{transport_begin, transport_confirm},
    },
    ws::ClientConnections,
};

static CONNECTIONS: OnceCell<ClientConnections> = OnceCell::const_new();
static BACKUP_REQUESTS: OnceCell<Queue> = OnceCell::const_new();
static AUTH_MANAGER: OnceCell<ClientAuthManager> = OnceCell::const_new();

#[tokio::main]
async fn main() {
    let bind_ip = dotenv::var("BIND_IP").expect("BIND_IP environment variable not set or invalid");
    let cert = dotenv::var("TLS_CERT").expect("TLS_CERT environment variable not set or invalid");
    let key = dotenv::var("TLS_KEY").expect("TLS_KEY environment variable not set or invalid");

    let db = Database::init().await;

    CONNECTIONS.set(ClientConnections::new()).unwrap();
    BACKUP_REQUESTS.set(Queue::new()).unwrap();
    AUTH_MANAGER.set(ClientAuthManager::new()).unwrap();

    let app = Route::new()
        .at("/register/begin", register_begin.data(db.clone()))
        .at("/register/complete", register_complete.data(db.clone()))
        .at("/login/begin", login_begin.data(db.clone()))
        .at("/login/complete", login_complete.data(db.clone()))
        .at("/backups/request", make_backup_request)
        .at("/backups/transport/begin", transport_begin.data(db.clone()))
        .at("/backups/transport/confirm", transport_confirm.data(db.clone()))
        .at("/ws", ws::handler.data(db));

    let config = RustlsConfig::new().fallback(RustlsCertificate::new().cert(cert).key(key));

    let listener = TcpListener::bind(bind_ip); //.rustls(config);
    let server = Server::new(listener);
    server.run(app).await.unwrap();
}
