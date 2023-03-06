mod ws;

use poem::{endpoint::EmbeddedFilesEndpoint, listener::TcpListener, EndpointExt, Route, Server};
use rust_embed::RustEmbed;
use tokio::sync::broadcast::Sender;

pub async fn run(log_sender: Sender<String>) {
    run_server(log_sender).await;
}

async fn run_server(log_sender: Sender<String>) {
    #[derive(RustEmbed)]
    #[folder = "static"]
    struct Static;

    let app = Route::new()
        .at("/", EmbeddedFilesEndpoint::<Static>::new())
        .at("/ws", ws::handler.data(log_sender));

    let listener = TcpListener::bind(crate::defaults::UI_BIND_IP);
    let server = Server::new(listener);
    server.run(app).await.unwrap();
}
