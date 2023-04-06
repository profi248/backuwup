mod backup;
mod identity;
mod peers;

use std::fs;

use sqlx::{
    sqlite::{SqlitePoolOptions, SqliteQueryResult},
    Error, Sqlite, SqlitePool,
};

#[derive(Clone, Debug)]
pub struct Config {
    db_pool: SqlitePool,
}

pub struct Transaction<'a> {
    transaction: sqlx::Transaction<'a, Sqlite>,
}

impl Config {
    pub async fn init() -> Self {
        let mut structure_initialized = true;

        let mut config_file = dirs::config_dir().expect("Cannot find the system config directory");
        config_file.push(crate::defaults::APP_FOLDER_NAME);

        fs::create_dir_all(config_file.clone()).unwrap_or_else(|_| {
            panic!("Unable write to the config folder {}", config_file.display())
        });

        config_file.push(crate::defaults::CONFIG_DB_FILE);

        if !config_file.try_exists().unwrap_or_else(|_| {
            panic!("Cannot access the config file at {}", config_file.display())
        }) {
            fs::File::create(config_file.clone()).unwrap_or_else(|_| {
                panic!("Unable to write the config file at {}", config_file.display())
            });
            structure_initialized = false;
        }

        let db_url = String::from("sqlite://")
            + config_file.to_str().unwrap_or_else(|| {
                panic!(
                    "The path to config file at {} contains invalid UTF-8 data",
                    config_file.display()
                )
            });

        let config = Self {
            db_pool: SqlitePoolOptions::new()
                .max_connections(5)
                .connect(&db_url)
                .await
                .unwrap_or_else(|_| {
                    panic!("Unable to open a config file at {}", config_file.display())
                }),
        };

        if !structure_initialized {
            Self::create_db_structure(&config.db_pool)
                .await
                .expect("Failed to create config database structure");
        }

        config
    }

    pub fn get_server_root_tls_cert(&self) -> Box<&[u8]> {
        Box::new(crate::defaults::SERVER_ROOT_TLS_CERT_PEM.as_bytes())
    }

    async fn create_db_structure(pool: &SqlitePool) -> Result<SqliteQueryResult, Error> {
        sqlx::query(
            "create table if not exists config
                (
                    key text not null
                        constraint config_pk
                        primary key,
                    value any
                );
            create table if not exists peers
                (
                    pubkey      blob not null
                        constraint peers_pk
                        primary key,
                    bytes_transmitted integer,
                    bytes_received    integer,
                    bytes_negotiated  integer,
                    first_seen        integer,
                    last_seen         integer
                );",
        )
        .execute(pool)
        .await
    }

    pub async fn transaction(&self) -> anyhow::Result<Transaction> {
        let transaction = self.db_pool.begin().await?;

        Ok(Transaction { transaction })
    }
}

impl Transaction<'_> {
    pub async fn commit(self) -> anyhow::Result<()> {
        self.transaction.commit().await?;
        Ok(())
    }

    pub async fn rollback(self) -> anyhow::Result<()> {
        self.transaction.rollback().await?;
        Ok(())
    }
}
