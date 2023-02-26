use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() {
    let db_url = dotenv::var("DB_URL").expect("DB_URL environment variable not set or invalid");

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&db_url).await.unwrap();


    // todo: run a websocket server here and pass the pool into the threads
}
