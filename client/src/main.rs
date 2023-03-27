#![deny(unused_must_use, deprecated)]
#![warn(clippy::pedantic)]
#![allow(clippy::redundant_else, clippy::too_many_lines, clippy::wrong_self_convention)]
#![allow(dead_code)]

mod backup;
mod cli;
mod config;
mod defaults;
mod identity;
mod key_manager;
mod net_p2p;
mod net_server;
mod packfile_receiver;
mod ui;

use std::{env, panic, process};

use futures_util::future;
use reqwest::{Certificate, Client};
use tokio::sync::{broadcast::channel, OnceCell};

use crate::{
    config::Config, key_manager::KeyManager, net_p2p::TransportRequestManager,
    net_server::requests, ui::logger::Logger,
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

    let ui_bind_addr = env::var("UI_BIND_ADDR").unwrap_or(defaults::UI_BIND_ADDR.to_string());

    let tasks = vec![tokio::spawn(net_server::connect_ws()), tokio::spawn(ui::run(ui_bind_addr))];

    // todo debug code
    // vvvvv
    if env::var("DEBUG_TRANSPORT").unwrap_or("0".to_string()) == "1" {
        println!("starting backup request");
        let cid = [
            42, 225, 115, 45, 238, 52, 91, 65, 171, 240, 177, 39, 64, 140, 204, 167, 29, 168, 4,
            116, 4, 14, 137, 22, 88, 223, 193, 253, 191, 39, 118, 118,
        ];
        let nonce = TRANSPORT_REQUESTS.get().unwrap().add_request(cid).await.unwrap();
        requests::backup_transport_begin(cid, nonce).await.unwrap();
    }

    if env::var("DEBUG_WALK").unwrap_or("0".to_string()) == "1" {
        //backup::filesystem_walker::walk().await.unwrap();
        backup::walker2::walk("/home/david/FIT/bachelors-thesis/backup-test".into()).await.unwrap();
        std::process::exit(0);
    }

    // ^^^^^

    future::join_all(tasks).await;
}
