//! Contains functions related to logging events.

use std::path::PathBuf;
use sqlx::Row;
use shared::types::ClientId;
use crate::config::{Config, Transaction};

enum EventType {
    RestoreRequest = 0,
    Backup = 1,
}

impl EventType {
    fn from_id(id: u8) -> Option<Self> {
        match id {
            0 => Some(EventType::RestoreRequest),
            1 => Some(EventType::Backup),
            _ => None,
        }
    }

    fn to_id(self) -> u8 {
        self as u8
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct RestoreRequestEvent {
    peer_id: ClientId,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct BackupEvent {
    size: i64,
    path: PathBuf,
}

impl Config {
    pub async fn log_peer_restore_request(&self, peer_id: ClientId) -> anyhow::Result<()> {
        let mut transaction = self.transaction().await?;
        let result = transaction.log_peer_restore_request(peer_id).await;
        transaction.commit().await?;

        result
    }

    pub async fn get_last_peer_restore_request(&self, peer_id: ClientId) -> anyhow::Result<Option<i64>> {
        let mut transaction = self.transaction().await?;
        let result = transaction.get_last_peer_restore_request(peer_id).await;
        transaction.commit().await?;

        result
    }

    pub async fn log_backup(&self, size: i64, path: &PathBuf) -> anyhow::Result<()> {
        let mut transaction = self.transaction().await?;
        let result = transaction.log_backup(size, path).await;
        transaction.commit().await?;

        result
    }

    pub async fn get_backup_size_difference(&self, size: i64, path: &PathBuf) -> anyhow::Result<Option<i64>> {
        let mut transaction = self.transaction().await?;
        let result = transaction.get_backup_size_difference(size, path).await;
        transaction.commit().await?;

        result
    }
}

impl Transaction<'_> {
    pub async fn log_peer_restore_request(&mut self, peer_id: ClientId) -> anyhow::Result<()> {
        let event = serde_json::to_string(&RestoreRequestEvent { peer_id })?;
        let event_type = EventType::RestoreRequest.to_id();

        sqlx::query(
            "insert into log (timestamp, event_type, event_data) values ($1, $2, json($3))",
        )
            .bind(Config::get_unix_timestamp())
            .bind(event_type)
            .bind(event)
            .execute(&mut self.transaction)
            .await?;

        Ok(())
    }

    pub async fn get_last_peer_restore_request(&mut self, peer_id: ClientId) -> anyhow::Result<Option<i64>> {
        let event_type = EventType::RestoreRequest.to_id();

        let result = sqlx::query(
            "select timestamp from log where event_type = $1 and event_data ->> 'peer_id' = $2 order by timestamp desc limit 1",
        )
            .bind(event_type)
            .bind(&peer_id[..])
            .fetch_optional(&mut self.transaction)
            .await?;

        match result {
            Some(row) => Ok(Some(row.try_get(0)?)),
            None => Ok(None),
        }
    }

    pub async fn log_backup(&mut self, size: i64, path: &PathBuf) -> anyhow::Result<()> {
        let event = serde_json::to_string(&BackupEvent { size, path: path.clone() })?;
        let event_type = EventType::Backup.to_id();

        sqlx::query(
            "insert into log (timestamp, event_type, event_data) values ($1, $2, json($3))",
        )
            .bind(Config::get_unix_timestamp())
            .bind(event_type)
            .bind(event)
            .execute(&mut self.transaction)
            .await?;

        Ok(())
    }

    pub async fn get_backup_size_difference(&mut self, size: i64, path: &PathBuf) -> anyhow::Result<Option<i64>> {
        let event_type = EventType::Backup.to_id();

        let result = sqlx::query(
            "select event_data ->> 'size' from log where event_type = $1 order by timestamp desc limit 1",
        )
            .bind(event_type)
            .fetch_optional(&mut self.transaction)
            .await?;

        match result {
            Some(row) => {
                let last_size: i64 = row.try_get(0)?;
                let old_path: String = row.try_get(1)?;
                let old_path = PathBuf::from(old_path);

                if old_path != *path { return Ok(None); }

                Ok(Some(size - last_size))
            },
            None => Ok(None),
        }
    }
}
