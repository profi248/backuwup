use std::path::PathBuf;

use sqlx::Row;

use crate::{
    config::{Config, Transaction},
    defaults::{APP_FOLDER_NAME, BACKUP_BUFFER_FOLDER_NAME},
};

impl Config {
    pub async fn set_backup_path(&self, path: PathBuf) -> anyhow::Result<()> {
        let mut transaction = self.transaction().await?;
        let result = transaction.set_backup_path(path).await;
        transaction.commit().await?;

        result
    }

    pub async fn get_backup_path(&self) -> anyhow::Result<Option<PathBuf>> {
        let mut transaction = self.transaction().await?;
        let result = transaction.get_backup_path().await;
        transaction.commit().await?;

        result
    }

    pub async fn get_packfile_path(&self) -> anyhow::Result<PathBuf> {
        let mut dir = dirs::data_local_dir().expect("Cannot find the system app data directory");
        dir.push(APP_FOLDER_NAME);
        dir.push(BACKUP_BUFFER_FOLDER_NAME);

        Ok(dir)
    }
}

impl Transaction<'_> {
    pub async fn set_backup_path(&mut self, path: PathBuf) -> anyhow::Result<()> {
        sqlx::query("insert or replace into config (key, value) values ('backup_path', ?)")
            .bind(path.to_string_lossy())
            .execute(&mut self.transaction)
            .await?;

        Ok(())
    }

    pub async fn get_backup_path(&mut self) -> anyhow::Result<Option<PathBuf>> {
        let path = sqlx::query("select value from config where key = 'backup_path'")
            .fetch_optional(&mut self.transaction)
            .await?
            .map(|row| row.get(0));

        Ok(path.map(|path: String| PathBuf::from(path)))
    }
}