//! This is the client part of the implementation. It is responsible for creating and restoring
//! backups, communicating with peers, sending metadata to the server and
//! providing a UI for the user.

#![deny(unused_must_use, deprecated)]
#![warn(clippy::pedantic)]
#![allow(
    dead_code,
    clippy::redundant_else,
    clippy::wrong_self_convention,
    clippy::manual_let_else,
    clippy::doc_markdown
)]

use std::{env, panic, process, time::Duration};

use enable_ansi_support::enable_ansi_support;
use futures_util::future;
use net_p2p::p2p_connection_manager::P2PConnectionManager;
use tokio::sync::{broadcast::channel, OnceCell};

use crate::{config::Config, key_manager::KeyManager, ui::ws_status_message::Messenger};

mod backup;
mod config;
mod defaults;
mod identity;
mod key_manager;
mod net_p2p;
mod net_server;
mod ui;

/// Keep track of outgoing P2P connection requests sent from different parts of the application.
static P2P_CONN_REQUESTS: OnceCell<P2PConnectionManager> = OnceCell::const_new();
/// Provides access to the global application configuration, which is used by many parts of the application.
static CONFIG: OnceCell<Config> = OnceCell::const_new();
/// Allows many parts different of the application to update state of the UI or send log messages.
static UI: OnceCell<Messenger> = OnceCell::const_new();
/// Generates and manages the secrets, all keys are derived from the root secret on startup.
static KEYS: OnceCell<KeyManager> = OnceCell::const_new();

#[tokio::main]
async fn main() {
    let config = Config::init().await;
    CONFIG.set(config.clone()).unwrap();

    // Windows needs explicit enabling of terminal color escapes support
    enable_ansi_support().ok();

    // make any panics in threads quit the entire application (https://stackoverflow.com/a/36031130),
    // and try sending send the panic message to the WebSocket clients
    let orig_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        // invoke the default handler and exit the process
        println!("Sorry, the program has encountered a fatal error and must exit. Please see details below:");
        orig_hook(panic_info);
        UI.get().unwrap().panic(panic_info.to_string());
        std::thread::sleep(Duration::from_secs(2));
        process::exit(1);
    }));

    if config.is_initialized().await.expect("Unable to open config") {
        // initialize the key manager with existing secret
        identity::load_secret().await.expect("Unable to load secret");
    } else {
        // first time setup is currently CLI and blocking
        ui::cli::first_run_guide().await;
    }

    // create a queue for sending all log messages to web clients
    let (log_sender, _) = channel(1000);
    UI.set(Messenger::new(log_sender.clone())).unwrap();

    P2P_CONN_REQUESTS.set(P2PConnectionManager::new()).unwrap();

    let ui_bind_addr = env::var("UI_BIND_ADDR").unwrap_or(defaults::UI_BIND_ADDR.to_string());

    let tasks = vec![tokio::spawn(net_server::connect_ws()), tokio::spawn(ui::run(ui_bind_addr))];

    future::join_all(tasks).await;
}
