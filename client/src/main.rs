#![deny(unused_must_use, deprecated)]
#![warn(clippy::pedantic)]

mod config;
mod defaults;
mod net;
mod ui;

use reqwest::{Certificate, Client};
use std::time::Duration;
use std::{panic, process};

use tokio::sync::broadcast::channel;
use tokio::time::sleep;

use crate::config::Config;

// todo: consider putting the log sender in a once_cell

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

    tokio::spawn(net::init());
    tokio::spawn(ui::run(log_sender.clone()));

    let client = Client::builder()
        .add_root_certificate(
            Certificate::from_pem(&config.get_server_root_tls_cert().await).unwrap(),
        )
        .tls_built_in_root_certs(false)
        .build()
        .unwrap();

    client.get("https://localhost:8080").send().await.unwrap();

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
