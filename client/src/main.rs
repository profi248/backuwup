#![deny(unused_must_use, deprecated)]
#![warn(clippy::pedantic)]

mod config;
mod defaults;
mod key_manager;
mod net;
mod ui;

use std::{panic, process, time::Duration};

use reqwest::{Certificate, Client};
use tokio::{
    sync::{broadcast::channel, OnceCell},
    time::sleep,
};

use crate::{config::Config, ui::logger::Logger};

static LOGGER: OnceCell<Logger> = OnceCell::const_new();

#[tokio::main]
async fn main() {
    let config = Config::init().await;

    // make any panics in threads quit the entire application (https://stackoverflow.com/a/36031130)
    let orig_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        // invoke the default handler and exit the process
        orig_hook(panic_info);
        process::exit(1);
    }));

    // create a queue for sending all log messages to web clients
    let (log_sender, _) = channel(100);
    LOGGER
        .set(Logger::new(log_sender.clone()))
        .expect("OnceCell failed");

    tokio::spawn(net::init());
    tokio::spawn(ui::run());

    let client = Client::builder()
        .add_root_certificate(Certificate::from_pem(&config.get_server_root_tls_cert()).unwrap())
        .tls_built_in_root_certs(false)
        .build()
        .unwrap();

    use local_ip_address::local_ip;

    let my_local_ip = local_ip().unwrap();
    println!("This is my local IP address: {:?}", my_local_ip);

    let mut i = 0;
    loop {
        let _ = log_sender.send(format!("helloo {i}"));
        i += 1;
        sleep(Duration::from_secs(2)).await;
    }
}
