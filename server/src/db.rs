use std::time::Duration;
use sqlx::{Error, PgPool, query};
use sqlx::postgres::{PgPoolOptions, PgQueryResult};

#[derive(Clone)]
pub struct Database {
    conn_pool: PgPool
}

impl Database {
    pub async fn init() -> Self {
        let db_url = dotenv::var("DB_URL").expect("DB_URL environment variable not set or invalid");

        let db = Self {
            conn_pool: PgPoolOptions::new()
                .max_connections(10)
                .acquire_timeout(Duration::from_secs(5))
                .connect(&db_url).await.expect(&format!("Failed to connect to database at {db_url}"))
        };

        println!("Connected to database!");
        Self::create_schema(db.conn_pool.clone()).await.expect("Failed to create database schema");
        db
    }

    async fn create_schema(pool: PgPool) -> Result<PgQueryResult, Error> {
        let schema = include_str!("schema/schema.sql");

        query(schema).execute(&pool).await
    }
}
