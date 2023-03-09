pub mod logger;
pub mod ws;

use poem::{endpoint::EmbeddedFilesEndpoint, listener::TcpListener, Route, Server};
use rust_embed::RustEmbed;

use crate::cli;

pub async fn run() {
    #[derive(RustEmbed)]
    #[folder = "static"]
    struct Static;

    let app = Route::new()
        .nest("/", EmbeddedFilesEndpoint::<Static>::new())
        .at("/ws", ws::handler);

    let listener = TcpListener::bind(crate::defaults::UI_BIND_IP);
    let server = Server::new(listener);

    cli::print_server_url(crate::defaults::UI_BIND_IP);
    server.run(app).await.unwrap();
}
