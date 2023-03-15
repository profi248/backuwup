#![deny(unused_must_use, deprecated)]
#![warn(clippy::pedantic)]
#![allow(clippy::redundant_else)]

mod cli;
mod config;
mod defaults;
mod identity;
mod key_manager;
mod net_p2p;
mod net_server;
mod ui;

use std::{panic, process};

use futures_util::future;
use net_p2p::receive;
use reqwest::{Certificate, Client};
use tokio::sync::{broadcast::channel, OnceCell};

use crate::{
    config::Config, key_manager::KeyManager, net_p2p::TransportRequestManager, ui::logger::Logger,
};

static TRANSPORT_REQUESTS: OnceCell<TransportRequestManager> = OnceCell::const_new();
static CONFIG: OnceCell<Config> = OnceCell::const_new();
static LOGGER: OnceCell<Logger> = OnceCell::const_new();
static KEYS: OnceCell<KeyManager> = OnceCell::const_new();

#[tokio::main]
async fn main() {
    let config = Config::init().await;
    CONFIG.set(config.clone()).unwrap();

    TRANSPORT_REQUESTS.set(TransportRequestManager::new()).unwrap();

    // make any panics in threads quit the entire application (https://stackoverflow.com/a/36031130)
    let orig_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        // invoke the default handler and exit the process
        orig_hook(panic_info);
        process::exit(1);
    }));

    if config.is_initialized().await.expect("Unable to open config") {
        // initialize the key manager with existing secret
        identity::load_secret().await.expect("Unable to load secret");
    } else {
        // first time setup is currently CLI and blocking
        cli::first_run_guide().await;
    }

    // create a queue for sending all log messages to web clients
    let (log_sender, _) = channel(100);
    LOGGER.set(Logger::new(log_sender.clone())).unwrap();

    let client = Client::builder()
        .add_root_certificate(Certificate::from_pem(&config.get_server_root_tls_cert()).unwrap())
        .tls_built_in_root_certs(false)
        .build()
        .unwrap();

    let (addr, port) = receive::get_listener_address().unwrap();
    println!("ip: {}", addr);
    tokio::spawn(receive::listen(port, Default::default(), Default::default()));

    let tasks = vec![tokio::spawn(net_server::connect_ws()), tokio::spawn(ui::run())];

    future::join_all(tasks).await;
}
