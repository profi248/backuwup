pub mod logger;
pub mod ws;

use poem::{endpoint::EmbeddedFilesEndpoint, listener::TcpListener, Route, Server};
use rust_embed::RustEmbed;

pub async fn run() {
    run_server().await;
}

async fn run_server() {
    #[derive(RustEmbed)]
    #[folder = "static"]
    struct Static;

    let app = Route::new()
        .at("/", EmbeddedFilesEndpoint::<Static>::new())
        .at("/ws", ws::handler);

    let listener = TcpListener::bind(crate::defaults::UI_BIND_IP);
    let server = Server::new(listener);
    server.run(app).await.unwrap();
}
