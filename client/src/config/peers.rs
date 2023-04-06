use std::path::PathBuf;

use anyhow::bail;
use shared::types::ClientId;
use sqlx::Row;

use super::{Config, Transaction};
use crate::defaults;

pub struct PeerInfo {
    pub pubkey: ClientId,
    pub bytes_transmitted: u64,
    pub bytes_received: u64,
    pub bytes_negotiated: u64,
    pub first_seen: u64,
    pub last_seen: u64,
}

impl Config {
    pub fn get_received_packfiles_folder(&self) -> anyhow::Result<PathBuf> {
        // todo allow the user to change this path (save to config)
        if let Some(mut directory) = dirs::data_local_dir() {
            directory.push(defaults::RECEIVED_PACKFILES_FOLDER);
            Ok(directory)
        } else {
            bail!("Unable to find system user data folder")
        }
    }

    pub async fn add_peer(&self, peer_id: ClientId, negotiated: u64) -> anyhow::Result<()> {
        let mut transaction = self.transaction().await?;
        transaction.add_peer(peer_id, negotiated).await?;
        transaction.commit().await?;

        Ok(())
    }

    pub async fn get_peer_info(&self, peer_id: ClientId) -> anyhow::Result<Option<PeerInfo>> {
        let mut transaction = self.transaction().await?;
        let peer = transaction.get_peer_info(peer_id).await?;
        transaction.commit().await?;

        Ok(peer)
    }

    pub async fn peer_increment_transmitted(
        &self,
        peer_id: ClientId,
        amount: u64,
    ) -> anyhow::Result<()> {
        let mut transaction = self.transaction().await?;
        transaction.peer_increment_transmitted(peer_id, amount).await?;
        transaction.commit().await?;

        Ok(())
    }

    pub async fn peer_increment_received(
        &self,
        peer_id: ClientId,
        amount: u64,
    ) -> anyhow::Result<()> {
        let mut transaction = self.transaction().await?;
        transaction.peer_increment_received(peer_id, amount).await?;
        transaction.commit().await?;

        Ok(())
    }

    pub async fn peer_get_negotiated_storage(&self, peer_id: ClientId) -> anyhow::Result<u64> {
        let mut transaction = self.transaction().await?;
        let amount = transaction.peer_get_negotiated_storage(peer_id).await?;
        transaction.commit().await?;

        Ok(amount)
    }

    pub async fn peer_set_negotiated_storage(&self, peer_id: ClientId, amount: u64) -> anyhow::Result<()> {
        let mut transaction = self.transaction().await?;
        transaction.peer_set_negotiated_storage(peer_id, amount).await?;
        transaction.commit().await?;

        Ok(())
    }

    pub async fn find_peers_with_storage(&self) -> anyhow::Result<Vec<ClientId>> {
        let mut transaction = self.transaction().await?;
        let peers = transaction.find_peers_with_storage().await?;
        transaction.commit().await?;

        Ok(peers)
    }
}

impl Transaction<'_> {
    pub async fn add_peer(&mut self, peer_id: ClientId, negotiated: u64) -> anyhow::Result<()> {
        sqlx::query(
            "insert into peers (pubkey, bytes_transmitted, bytes_received, bytes_negotiated, first_seen, last_seen)
                values ($1, 0, 0, $2, now(), now())",
        )
        .bind(&peer_id[..])
        .bind(negotiated as i64)
        .execute(&mut self.transaction)
        .await?;

        Ok(())
    }

    pub async fn get_peer_info(&mut self, peer_id: ClientId) -> anyhow::Result<Option<PeerInfo>> {
        let peer = sqlx::query(
            "select bytes_transmitted, bytes_received, bytes_negotiated, first_seen, last_seen from peers where pubkey = $1",
        )
        .bind(&peer_id[..])
        .fetch_optional(&mut self.transaction)
        .await?;

        match peer {
            Some(row) => {
                let bytes_transmitted: i64 = row.try_get(0)?;
                let bytes_received: i64 = row.try_get(1)?;
                let bytes_negotiated: i64 = row.try_get(2)?;
                let first_seen: i64 = row.try_get(3)?;
                let last_seen: i64 = row.try_get(4)?;

                Ok(Some(PeerInfo {
                    pubkey: peer_id,
                    bytes_transmitted: bytes_transmitted as u64,
                    bytes_received: bytes_received as u64,
                    bytes_negotiated: bytes_negotiated as u64,
                    first_seen: first_seen as u64,
                    last_seen: last_seen as u64,
                }))
            }
            None => Ok(None),
        }
    }

    pub async fn peer_increment_transmitted(
        &mut self,
        peer_id: ClientId,
        amount: u64,
    ) -> anyhow::Result<()> {
        sqlx::query("update peers set bytes_transmitted = bytes_transmitted + $1, last_seen = now() where pubkey = $2")
            .bind(amount as i64)
            .bind(&peer_id[..])
            .execute(&mut self.transaction)
            .await?;

        Ok(())
    }

    pub async fn peer_increment_received(
        &mut self,
        peer_id: ClientId,
        amount: u64,
    ) -> anyhow::Result<()> {
        sqlx::query("update peers set bytes_received = bytes_received + $1, last_seen = now() where pubkey = $2")
            .bind(amount as i64)
            .bind(&peer_id[..])
            .execute(&mut self.transaction)
            .await?;

        Ok(())
    }

    pub async fn peer_get_negotiated_storage(&mut self, peer_id: ClientId) -> anyhow::Result<u64> {
        Ok(sqlx::query("select bytes_negotiated from peers where pubkey = $1")
            .bind(&peer_id[..])
            .fetch_one(&mut self.transaction)
            .await?
            .try_get(0)
            .map(|val: i64| val as u64)?)
    }

    pub async fn peer_set_negotiated_storage(&mut self, peer_id: ClientId, amount: u64) -> anyhow::Result<()> {
        sqlx::query("update peers set bytes_negotiated = $1 where pubkey = $2")
            .bind(amount as i64)
            .bind(&peer_id[..])
            .execute(&mut self.transaction)
            .await?;

        Ok(())
    }

    pub async fn find_peers_with_storage(&mut self) -> anyhow::Result<Vec<ClientId>> {
        let rows = sqlx::query(
            "select pubkey, (bytes_negotiated - bytes_transmitted) as free_storage \
             from peers where free_storage > 0 \
             order by free_storage desc")
            .fetch_all(&mut self.transaction)
            .await?;

        let mut peers: Vec<ClientId> = Vec::new();
        for row in rows {
            let peer_id: &[u8] = row.try_get(0)?;
            peers.push(peer_id.try_into()?);
        }

        Ok(peers)
    }
}
