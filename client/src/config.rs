use anyhow::anyhow;
use std::fs;
use std::sync::Arc;

use sqlx::{sqlite::{SqlitePoolOptions, SqliteQueryResult}, Error, Row, SqlitePool, Transaction, Sqlite};

use crate::key_manager::MasterSecret;

#[derive(Clone, Debug)]
pub struct Config {
    db_pool: SqlitePool,
}

pub struct ConfigTransaction<'a> {
    config: Config,
    transaction: Transaction<'a, Sqlite>,
}

impl Config {
    pub async fn init() -> Self {
        let mut structure_initialized = true;

        let mut config_file = dirs::config_dir().expect("Cannot find the system config directory");
        config_file.push(crate::defaults::CONFIG_FOLDER);

        fs::create_dir_all(config_file.clone())
            .expect(&format!("Unable write to the config folder {}", config_file.display()));

        config_file.push(crate::defaults::CONFIG_DB_FILE);

        if !config_file
            .try_exists()
            .expect(&format!("Cannot access the config file at {}", config_file.display()))
        {
            fs::File::create(config_file.clone())
                .expect(&format!("Unable to write the config file at {}", config_file.display()));
            structure_initialized = false;
        }

        let db_url = String::from("sqlite://")
            + config_file.to_str().expect(&format!(
            "The path to config file at {} contains invalid UTF-8 data",
            config_file.display()
        ));

        let config = Self {
            db_pool: SqlitePoolOptions::new()
                .max_connections(5)
                .connect(&db_url)
                .await
                .expect(&format!("Unable to open a config file at {}", config_file.display())),
        };

        if !structure_initialized {
            Self::create_db_structure(&config.db_pool)
                .await
                .expect("Failed to create config database structure");
        }

        config
    }

    pub fn get_server_root_tls_cert(&self) -> Box<&[u8]> {
        Box::new(crate::defaults::SERVER_ROOT_TLS_CERT_PEM.clone().as_bytes())
    }

    async fn create_db_structure(pool: &SqlitePool) -> Result<SqliteQueryResult, Error> {
        sqlx::query(
            "create table if not exists config
                (
                    key TEXT not null
                        constraint config_pk
                        primary key,
                    value ANY
                );",
        )
            .execute(pool)
            .await
    }

    pub async fn transaction(&self) -> anyhow::Result<ConfigTransaction> {
        let transaction = self.db_pool.begin().await?;
        Ok(
            ConfigTransaction {
                config: self.clone(),
                transaction
            }
        )
    }

    pub async fn is_initialized(&self) -> anyhow::Result<bool> {
        let initalized = sqlx::query("select value from config where key = 'initialized'")
            .fetch_optional(&self.db_pool)
            .await?
            .is_some();

        Ok(initalized)
    }
}

impl<'a> ConfigTransaction<'a> {
    pub async fn commit(self) -> anyhow::Result<()> {
        self.transaction.commit().await?;
        Ok(())
    }

    pub async fn rollback(self) -> anyhow::Result<()> {
        self.transaction.rollback().await?;
        Ok(())
    }

    pub async fn is_initialized(&mut self) -> anyhow::Result<bool> {
        let initalized = sqlx::query("select value from config where key = 'initialized'")
            .fetch_optional(&mut self.transaction)
            .await?
            .is_some();

        Ok(initalized)
    }

    pub async fn set_initialized(&mut self) -> anyhow::Result<()> {
        sqlx::query("insert into config (key, value) values ('initialized', 1)")
            .execute(&mut self.transaction)
            .await?;
        Ok(())
    }

    pub async fn save_master_secret(&mut self, secret: MasterSecret) -> anyhow::Result<()> {
        sqlx::query("insert into config (key, value) values ('master_secret', $1)")
            .bind(Vec::from(secret))
            .execute(&mut self.transaction)
            .await?;
        Ok(())
    }

    pub async fn load_master_secret(&mut self) -> anyhow::Result<MasterSecret> {
        let secret: Vec<u8> = sqlx::query("select value from config where key = 'master_secret'")
            .fetch_one(&mut self.transaction)
            .await?
            .get(0);

        match secret.try_into() {
            Ok(secret) => Ok(secret),
            Err(_) => Err(anyhow!("Invalid master secret")),
        }
    }
}
