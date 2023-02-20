use std::thread::sleep_ms;
use std::time::Duration;
use async_channel::{Receiver, Sender};
use console::Term;
use tokio::time::sleep;
use dialoguer::{ theme::ColorfulTheme, Input };

#[tokio::main]
async fn main() {
    let (input_sender, input_receiver) = async_channel::unbounded();
    let (server_sender, server_receiver) = async_channel::unbounded();
    let (evt_sender, evt_receiver) = async_channel::unbounded();


    tokio::spawn(server(evt_sender.clone(), server_receiver.clone()));
    // tokio::spawn(waker(queue_sender.clone(), queue_receiver.clone()));


    // use evt_receiver from all threads, or reconsider using a different queue type
    loop {
        tokio::spawn(input_handler(input_sender.clone(), input_receiver.clone()));
        let msg = input_receiver.recv().await.unwrap();

        server_sender.send(msg).await;
        evt_receiver.recv().await.unwrap();
    }


}

async fn input_handler(queue_sender: Sender<String>, queue_receiver: Receiver<String>) {
    let input: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Your name")
        .interact_text()
        .unwrap();

    queue_sender.send(input).await;
}

async fn server(queue_sender: Sender<String>, queue_receiver: Receiver<String>) {
    loop {
        let string = queue_receiver.recv().await.unwrap();
        println!("hii {string} :3");

        let bar = indicatif::ProgressBar::new(100);
        for _ in 0..100 {
            bar.inc(1);
            sleep(Duration::from_millis(10)).await;
        }

        bar.finish();
        queue_sender.send("a".to_string()).await;
    }
}

async fn waker(queue_sender: Sender<String>, queue_receiver: Receiver<String>) {
    loop {
        // queue_sender.send("hii from waker :3".to_string()).await;
        let term = Term::stdout();
        term.write_line("hii from waker :3");
        sleep(Duration::from_secs(2)).await;
    }
}
