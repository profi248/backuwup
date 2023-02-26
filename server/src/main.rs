mod ws;

use base64::Engine;
use base64::engine::general_purpose;
use sqlx::postgres::PgPoolOptions;
use poem::{Route, listener::TcpListener, Server, EndpointExt};
use poem::listener::{Listener, RustlsCertificate, RustlsConfig};

#[tokio::main]
async fn main() {
    let db_url = dotenv::var("DB_URL").expect("DB_URL environment variable not set or invalid");
    let bind_ip = dotenv::var("BIND_IP").expect("BIND_IP environment variable not set or invalid");
    let cert = dotenv::var("TLS_CERT").expect("TLS_CERT environment variable not set or invalid");
    let key = dotenv::var("TLS_KEY").expect("TLS_KEY environment variable not set or invalid");

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&db_url).await.unwrap();

    let app = Route::new()
        .at("/ws", ws::handler.data(pool));

    let config =
        RustlsConfig::new().fallback(RustlsCertificate::new().cert(cert).key(key));

    let listener = TcpListener::bind(bind_ip)
        .rustls(config);
    let server = Server::new(listener);
    server.run(app).await.unwrap();
}
