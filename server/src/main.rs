mod ws;

use sqlx::postgres::PgPoolOptions;
use poem::{Route, listener::TcpListener, Server, EndpointExt};

#[tokio::main]
async fn main() {
    let db_url = dotenv::var("DB_URL").expect("DB_URL environment variable not set or invalid");
    let bind_ip = dotenv::var("BIND_IP").expect("BIND_IP environment variable not set or invalid");

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&db_url).await.unwrap();

    let app = Route::new()
        .at("/ws", ws::handler.data(pool));

    let listener = TcpListener::bind(bind_ip);
    let server = Server::new(listener);
    server.run(app).await.unwrap();
}
