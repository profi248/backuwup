#![deny(unused_must_use, deprecated)]
#![warn(clippy::pedantic)]

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

    CONNECTIONS
        .set(ClientConnections::new())
        .expect("OnceCell failed");

    BACKUP_REQUESTS.set(Queue::new()).expect("OnceCell failed");

    AUTH_MANAGER
        .set(ClientAuthManager::new())
        .expect("OnceCell failed");

    let app = Route::new()
        .at("/register/begin", register_begin)
        .at("/register/complete", register_complete)
        .at("/login/begin", login_begin)
        .at("/login/complete", login_complete)
        .at("/backups/request", make_backup_request)
        .at("/ws", ws::handler.data(db));

    let config = RustlsConfig::new().fallback(RustlsCertificate::new().cert(cert).key(key));

    let listener = TcpListener::bind(bind_ip); //.rustls(config);
    let server = Server::new(listener);
    server.run(app).await.unwrap();
}
