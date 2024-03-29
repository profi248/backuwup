//! Contains the backend for the UI.

pub mod cli;
pub mod ws;
mod ws_dispatcher;
pub mod ws_status_message;

use poem::{endpoint::EmbeddedFilesEndpoint, listener::TcpListener, Route, Server};
use rust_embed::RustEmbed;

/// Serve the static files and register the WebSocket endpoint.
pub async fn run(bind_addr: String) {
    #[derive(RustEmbed)]
    #[folder = "static"]
    struct Static;

    let app = Route::new()
        .nest("/", EmbeddedFilesEndpoint::<Static>::new())
        .at("/ws", ws::handler);

    let listener = TcpListener::bind(&bind_addr);
    let server = Server::new(listener);

    cli::print_server_url(&bind_addr);
    server.run(app).await.unwrap();
}
