use std::{path::Path, fs};

use sqlx::sqlite::{SqlitePoolOptions, SqliteQueryResult};
use sqlx::{Error, SqlitePool};

#[derive(Clone)]
pub(crate) struct Config {
    db_pool: SqlitePool
}

impl Config {
    pub async fn init() -> Self {
        let mut structure_initialized = true;

        let mut config_file = dirs::config_dir().expect("Cannot find the system config directory");
        config_file.push(crate::defaults::CONFIG_FOLDER);

        fs::create_dir_all(config_file.clone()).expect(&format!("Unable write to the config folder {}", config_file.display()));

        config_file.push(crate::defaults::CONFIG_DB_FILE);

        if !config_file.try_exists().expect(&format!("Cannot access the config file at {}", config_file.display())) {
            fs::File::create(config_file.clone()).expect(&format!("Unable to write the config file at {}", config_file.display()));
            structure_initialized = false;
        }

        let db_url = String::from("sqlite://") + config_file.to_str()
            .expect(&format!("The path to config file at {} contains invalid UTF-8 data", config_file.display()));

        let config = Self {
            db_pool: SqlitePoolOptions::new()
                .max_connections(5)
                .connect(&db_url).await.expect(&format!("Unable to open a config file at {}", config_file.display()))
        };

        if !structure_initialized {
            Self::create_db_structure(&config.db_pool).await
                .expect("Failed to create config database structure");
        }

        config
    }

    pub async fn get_server_root_tls_cert(&self) -> Box<&[u8]> {
        Box::new(crate::defaults::SERVER_ROOT_TLS_CERT_PEM.clone().as_bytes())
    }

    async fn create_db_structure(pool: &SqlitePool) -> Result<SqliteQueryResult, Error> {
        sqlx::query("create table if not exists config
                (
                    key TEXT not null
                        constraint config_pk
                        primary key,
                    value ANY
                );").execute(pool).await
    }
}
