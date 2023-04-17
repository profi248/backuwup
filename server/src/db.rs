use std::time::Duration;

use shared::types::{BlobHash, ClientId};
use sqlx::{
    postgres::{PgPoolOptions, PgRow},
    query, Executor, PgPool, Postgres, Row,
};

use crate::handlers;

#[derive(Clone, Debug)]
pub struct Database {
    conn_pool: PgPool,
}

impl Database {
    pub async fn init() -> Self {
        let db_url = dotenvy::var("DB_URL").expect("DB_URL environment variable not set or invalid");

        let mut attempts = 5;
        let db = loop {
            let pool = PgPoolOptions::new()
                .max_connections(10)
                .acquire_timeout(Duration::from_secs(10))
                .connect(&db_url)
                .await;

            match pool {
                Ok(p) => break Self { conn_pool: p },
                Err(e) => {
                    attempts -= 1;
                    if attempts == 0 {
                        panic!("unable to connect to database after 5 attempts");
                    }
                    println!("connecting to database failed: {e}, will try {attempts} more times");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
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

    pub async fn client_update_logged_in(&self, client_id: ClientId) -> Result<(), handlers::Error> {
        query("update clients set last_login = now() where pubkey = $1")
            .bind(client_id)
            .execute(&self.conn_pool)
            .await?;

        Ok(())
    }

    pub async fn save_storage_negotiated(
        &self,
        source: ClientId,
        destination: ClientId,
        storage_negotiated: i64,
    ) -> Result<(), handlers::Error> {
        query(
            "insert into peer_backups (source, destination, size_negotiated, timestamp)
               values ($1, $2, $3, now())",
        )
        .bind(source)
        .bind(destination)
        .bind(storage_negotiated)
        .execute(&self.conn_pool)
        .await?;

        Ok(())
    }

    pub async fn get_latest_client_snapshot(
        &self,
        client_id: ClientId,
    ) -> Result<Option<BlobHash>, handlers::Error> {
        let result = query(
            "select snapshot_hash from snapshots where client_pubkey = $1 \
                                          order by timestamp desc limit 1",
        )
        .bind(client_id)
        .fetch_optional(&self.conn_pool)
        .await?;

        match result {
            Some(val) => Ok(Some(
                val.get::<Vec<u8>, usize>(0)
                    .try_into()
                    .map_err(|_| handlers::Error::DatabaseTypeMismatch)?,
            )),
            None => Ok(None),
        }
    }

    pub async fn save_snapshot(
        &self,
        client_id: ClientId,
        snapshot_hash: BlobHash,
    ) -> Result<(), handlers::Error> {
        query("insert into snapshots (client_pubkey, snapshot_hash, timestamp) values ($1, $2, now())")
            .bind(client_id)
            .bind(snapshot_hash)
            .execute(&self.conn_pool)
            .await?;

        Ok(())
    }

    pub async fn get_client_negotiated_peers(
        &self,
        client_id: ClientId,
    ) -> Result<Vec<ClientId>, handlers::Error> {
        let mut result: Vec<ClientId> = Vec::new();
        let rows = query("select destination from peer_backups where source = $1")
            .bind(client_id)
            .fetch_all(&self.conn_pool)
            .await?;

        for row in rows {
            let client_id: Vec<u8> = row.get(0);
            result.push(
                client_id
                    .try_into()
                    .map_err(|_| handlers::Error::DatabaseTypeMismatch)?,
            );
        }

        Ok(result)
    }
}
