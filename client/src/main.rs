#![deny(unused_must_use, deprecated)]
#![warn(clippy::pedantic)]

mod net;
mod ui;
mod defaults;
mod config;

use std::{panic, process};
use std::time::Duration;
use tokio::time::sleep;

use tokio::sync::broadcast::channel;

// todo: consider putting the log sender in a once_cell

#[tokio::main]
async fn main() {
    let config = crate::config::Config::init().await;
    let c = config.clone();

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

    let mut i = 0;
    loop {
        let _ = log_sender.send(format!("helloo {i}"));
        i += 1;
        sleep(Duration::from_secs(2)).await;
    }
}
