use std::time::Duration;

use shared::types::ClientId;
use sqlx::{postgres::PgPoolOptions, query, Executor, PgPool};

use crate::handlers;

#[derive(Clone)]
pub struct Database {
    conn_pool: PgPool,
}

impl Database {
    pub async fn init() -> Self {
        let db_url = dotenv::var("DB_URL").expect("DB_URL environment variable not set or invalid");

        let db = Self {
            conn_pool: PgPoolOptions::new()
                .max_connections(10)
                .acquire_timeout(Duration::from_secs(5))
                .connect(&db_url)
                .await
                .expect(&format!("Failed to connect to database at {db_url}")),
        };

        println!("[db] connected to database");
        Self::create_schema(db.conn_pool.clone())
            .await
            .expect("Failed to create database schema");
        db
    }

    async fn create_schema(pool: PgPool) -> Result<(), handlers::Error> {
        let result = query("select value from metadata where key = 'schema_version'")
            .fetch_optional(&pool)
            .await;

        // error are matched, because the schema might not exist yet
        match result {
            Ok(Some(_)) => {
                // for now just check whether the schema has been created
                Ok(())
            }
            Err(_) | Ok(None) => {
                println!("[db] creating schema");

                let schema = include_str!("schema/schema.sql");
                (&pool).execute(schema).await?;

                query("insert into metadata (key, value) values ('schema_version', '1')")
                    .execute(&pool)
                    .await?;

                Ok(())
            }
        }
    }

    pub async fn register_client(&self, client_id: ClientId) -> Result<(), handlers::Error> {
        query("insert into clients (pubkey, registered) values ($1, now())")
            .bind(client_id)
            .execute(&self.conn_pool)
            .await?;

        Ok(())
    }

    pub async fn client_exists(&self, client_id: ClientId) -> Result<bool, handlers::Error> {
        let result = query("select pubkey from clients where pubkey = $1")
            .bind(client_id)
            .fetch_optional(&self.conn_pool)
            .await
            .map(|result| result.is_some())?;

        Ok(result)
    }
}
