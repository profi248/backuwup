#![deny(unused_must_use, deprecated)]
#![warn(clippy::pedantic)]
#![allow(
    clippy::redundant_else,
    clippy::too_many_lines,
    clippy::wrong_self_convention,
    clippy::manual_let_else
)]

//#![allow(dead_code)]

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
use std::time::Duration;

use futures_util::future;
use reqwest::{Certificate, Client};
use tokio::sync::{broadcast::channel, OnceCell};

use crate::{
    config::Config, key_manager::KeyManager, net_p2p::TransportRequestManager,
    net_server::requests, ui::ws_status_message::Messenger,
};

static TRANSPORT_REQUESTS: OnceCell<TransportRequestManager> = OnceCell::const_new();
static CONFIG: OnceCell<Config> = OnceCell::const_new();
static UI: OnceCell<Messenger> = OnceCell::const_new();
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
        UI.get().unwrap().panic(panic_info.to_string());
        std::thread::sleep(Duration::from_secs(2));
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
    UI.set(Messenger::new(log_sender.clone())).unwrap();

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
            50, 84, 68, 172, 255, 104, 247, 131, 137, 207, 210, 193, 128, 205, 245, 225, 194, 255,
            92, 228, 115, 173, 104, 149, 82, 139, 151, 45, 213, 103, 112, 3,
        ];
        let nonce = TRANSPORT_REQUESTS.get().unwrap().add_request(cid).await.unwrap();
        requests::backup_transport_begin(cid, nonce).await.unwrap();
    }

    if env::var("DEBUG_BACKUP").unwrap_or("0".to_string()) == "1" {
        //backup::filesystem_walker::walk().await.unwrap();
        let hash = backup::filesystem::dir_packer::pack(
            "/home/david/backup-testing/source".into(),
            "/home/david/backup-testing/packs".into(),
        )
        .await
        .unwrap();

        println!("backup done, snapshot hash: {}", hex::encode(hash));
        std::process::exit(0);
    }

    if let Ok(hash) = env::var("DEBUG_RESTORE") {
        println!("restoring...");
        backup::filesystem::dir_unpacker::unpack(
            "/home/david/backup-testing/packs",
            "/home/david/backup-testing/restored",
            hex::decode(hash).unwrap().try_into().unwrap(),
        )
        .await
        .unwrap();
        std::process::exit(0);
    }

    // ^^^^^

    future::join_all(tasks).await;
}
