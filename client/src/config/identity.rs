use anyhow::anyhow;
use shared::types::SessionToken;
use sqlx::Row;

use super::{Config, Transaction};
use crate::key_manager::MasterSecret;

impl Config {
    pub async fn is_initialized(&self) -> anyhow::Result<bool> {
        let mut transaction = self.transaction().await?;
        let result = transaction.is_initialized().await;
        transaction.commit().await?;

        result
    }

    pub async fn set_initialized(&self) -> anyhow::Result<()> {
        let mut transaction = self.transaction().await?;
        let result = transaction.set_initialized().await;
        transaction.commit().await?;

        result
    }

    pub async fn save_master_secret(&self, secret: MasterSecret) -> anyhow::Result<()> {
        let mut transaction = self.transaction().await?;
        let result = transaction.save_master_secret(secret).await;
        transaction.commit().await?;

        result
    }

    pub async fn load_master_secret(&self) -> anyhow::Result<MasterSecret> {
        let mut transaction = self.transaction().await?;
        let result = transaction.load_master_secret().await;
        transaction.commit().await?;

        result
    }

    pub async fn save_auth_token(&self, token: Option<SessionToken>) -> anyhow::Result<()> {
        let mut transaction = self.transaction().await?;
        let result = transaction.save_auth_token(token).await;
        transaction.commit().await?;

        result
    }

    pub async fn load_auth_token(&self) -> anyhow::Result<Option<SessionToken>> {
        let mut transaction = self.transaction().await?;
        let result = transaction.load_auth_token().await;
        transaction.commit().await?;

        result
    }

    pub async fn save_obfuscation_key(&self, key: u32) -> anyhow::Result<()> {
        let mut transaction = self.transaction().await?;
        let result = transaction.save_obfuscation_key(key).await;
        transaction.commit().await?;

        result
    }

    pub async fn get_obfuscation_key(&self) -> anyhow::Result<u32> {
        let mut transaction = self.transaction().await?;
        let result = transaction.get_obfuscation_key().await;
        transaction.commit().await?;

        result
    }
}

impl Transaction<'_> {
    pub async fn is_initialized(&mut self) -> anyhow::Result<bool> {
        let initialized = sqlx::query("select value from config where key = 'initialized'")
            .fetch_optional(&mut self.transaction)
            .await?
            .is_some();

        Ok(initialized)
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

    pub async fn save_auth_token(&mut self, token: Option<SessionToken>) -> anyhow::Result<()> {
        if let Some(token) = token {
            sqlx::query("insert or replace into config (key, value) values ('auth_token', $1)")
                .bind(Vec::from(token))
                .execute(&mut self.transaction)
                .await?;
        } else {
            sqlx::query("delete from config where key = 'auth_token'")
                .execute(&mut self.transaction)
                .await?;
        }

        Ok(())
    }

    pub async fn load_auth_token(&mut self) -> anyhow::Result<Option<SessionToken>> {
        let token = sqlx::query("select value from config where key = 'auth_token'")
            .fetch_optional(&mut self.transaction)
            .await?;

        match token {
            Some(row) => {
                let token: SessionToken = row
                    .try_get::<Vec<u8>, usize>(0)?
                    .try_into()
                    .map_err(|_| anyhow!("Invalid auth token length"))?;
                Ok(Some(token))
            }
            None => Ok(None),
        }
    }

    pub async fn save_obfuscation_key(&mut self, key: u32) -> anyhow::Result<()> {
        sqlx::query("insert into config (key, value) values ('obfuscation_key', $1)")
            .bind(Vec::from(key.to_le_bytes()))
            .execute(&mut self.transaction)
            .await?;

        Ok(())
    }

    pub async fn get_obfuscation_key(&mut self) -> anyhow::Result<u32> {
        let key: Vec<u8> = sqlx::query("select value from config where key = 'obfuscation_key'")
            .fetch_one(&mut self.transaction)
            .await?
            .get(0);

        let key = u32::from_le_bytes(key.try_into().map_err(|_| anyhow!("invalid obfuscation key"))?);
        Ok(key)
    }
}
