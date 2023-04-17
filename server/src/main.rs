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
        backup::{backup_done, backup_restore},
        backup_request::make_backup_request,
        login::{login_begin, login_complete},
        p2p_connection_request::{p2p_connection_begin, p2p_connection_confirm},
        register::{register_begin, register_complete},
    },
    ws::ClientConnections,
};

static DB: OnceCell<Database> = OnceCell::const_new();
static CONNECTIONS: OnceCell<ClientConnections> = OnceCell::const_new();
static BACKUP_REQUESTS: OnceCell<Queue> = OnceCell::const_new();
static AUTH_MANAGER: OnceCell<ClientAuthManager> = OnceCell::const_new();

#[tokio::main]
async fn main() {
    let bind_ip = dotenvy::var("BIND_IP").expect("BIND_IP environment variable not set or invalid");
    let cert = dotenvy::var("TLS_CERT").expect("TLS_CERT environment variable not set or invalid");
    let key = dotenvy::var("TLS_KEY").expect("TLS_KEY environment variable not set or invalid");

    let db = Database::init().await;

    DB.set(db).unwrap();
    CONNECTIONS.set(ClientConnections::new()).unwrap();
    BACKUP_REQUESTS.set(Queue::new()).unwrap();
    AUTH_MANAGER.set(ClientAuthManager::new()).unwrap();

    let app = Route::new()
        .at("/register/begin", register_begin)
        .at("/register/complete", register_complete)
        .at("/login/begin", login_begin)
        .at("/login/complete", login_complete)
        .at("/backups/request", make_backup_request)
        .at("/backups/done", backup_done)
        .at("/backups/restore", backup_restore)
        .at("/p2p/connection/begin", p2p_connection_begin)
        .at("/p2p/connection/confirm", p2p_connection_confirm)
        .at("/ws", ws::handler);

    let config = RustlsConfig::new().fallback(RustlsCertificate::new().cert(cert).key(key));

    let listener = TcpListener::bind(&bind_ip); //.rustls(config);
    let server = Server::new(listener);
    println!("server listening on {bind_ip}");
    server.run(app).await.unwrap();
}
