//! Configuration related to peer communication and files received from peers.

use std::path::PathBuf;

use shared::types::ClientId;
use sqlx::Row;

use super::{Config, Transaction};
use crate::{defaults, defaults::PEER_STORAGE_USAGE_SPREAD};

/// Struct containing information about a peer.
pub struct PeerInfo {
    pub pubkey: ClientId,
    pub bytes_transmitted: i64,
    pub bytes_received: i64,
    pub bytes_negotiated: i64,
    pub first_seen: i64,
    pub last_seen: i64,
}

impl Config {
    /// Gets the directory where received packfiles are stored.
    pub fn get_received_packfiles_folder(&self) -> anyhow::Result<PathBuf> {
        let mut dir = Config::get_data_dir()?;
        dir.push(defaults::APP_FOLDER_NAME);
        dir.push(defaults::RECEIVED_PACKFILES_FOLDER);

        Ok(dir)
    }

    /// Gets the directory where temporary restore packfiles are stored.
    pub fn get_restored_packfiles_folder(&self) -> anyhow::Result<PathBuf> {
        let mut dir = Config::get_data_dir()?;
        dir.push(defaults::APP_FOLDER_NAME);
        dir.push(defaults::RESTORE_BUFFER_FOLDER);

        Ok(dir)
    }

    /// Adds a peer or increments the peer's negotiated storage.
    pub async fn add_or_increment_peer_storage(
        &self,
        peer_id: ClientId,
        negotiated: u64,
    ) -> anyhow::Result<()> {
        let mut transaction = self.transaction().await?;
        transaction.add_or_increment_peer(peer_id, negotiated).await?;
        transaction.commit().await?;

        Ok(())
    }

    /// Get complete information about a peer.
    pub async fn get_peer_info(&self, peer_id: ClientId) -> anyhow::Result<Option<PeerInfo>> {
        let mut transaction = self.transaction().await?;
        let peer = transaction.get_peer_info(peer_id).await?;
        transaction.commit().await?;

        Ok(peer)
    }

    /// Increment peer's transmitted bytes.
    pub async fn peer_increment_transmitted(&self, peer_id: ClientId, amount: u64) -> anyhow::Result<()> {
        let mut transaction = self.transaction().await?;
        transaction.peer_increment_transmitted(peer_id, amount).await?;
        transaction.commit().await?;

        Ok(())
    }

    /// Increment peer's received bytes.
    pub async fn peer_increment_received(&self, peer_id: ClientId, amount: u64) -> anyhow::Result<()> {
        let mut transaction = self.transaction().await?;
        transaction.peer_increment_received(peer_id, amount).await?;
        transaction.commit().await?;

        Ok(())
    }

    /// Get peers that have negotiated storage available.
    pub async fn find_peers_with_storage(&self) -> anyhow::Result<Vec<ClientId>> {
        let mut transaction = self.transaction().await?;
        let peers = transaction.find_peers_with_storage().await?;
        transaction.commit().await?;

        Ok(peers)
    }

    /// Get all peers with, optionally, last seen time within the given limit.
    pub async fn get_peers(&self, last_seen_limit_seconds: Option<u64>) -> anyhow::Result<Vec<PeerInfo>> {
        let mut transaction = self.transaction().await?;
        let peers = transaction.get_peers(last_seen_limit_seconds).await?;
        transaction.commit().await?;

        Ok(peers)
    }

    /// Update peer's last seen time.
    pub async fn peer_update_last_seen(&self, peer_id: ClientId) -> anyhow::Result<()> {
        let mut transaction = self.transaction().await?;
        transaction.peer_update_last_seen(peer_id).await?;
        transaction.commit().await?;

        Ok(())
    }
}

impl Transaction<'_> {
    /// Adds a peer or increments the peer's negotiated storage.
    pub async fn add_or_increment_peer(&mut self, peer_id: ClientId, negotiated: u64) -> anyhow::Result<()> {
        sqlx::query(
            "insert into peers (pubkey, bytes_transmitted, bytes_received, bytes_negotiated, first_seen, last_seen)
                values ($1, 0, 0, $2, $3, $3)
                on conflict (pubkey) do update set bytes_negotiated = peers.bytes_negotiated + $2, last_seen = $3",
        )
        .bind(&peer_id[..])
        .bind(negotiated as i64)
        .bind(Config::get_unix_timestamp())
        .execute(&mut self.transaction)
        .await?;

        Ok(())
    }

    /// Get complete information about a peer.
    pub async fn get_peer_info(&mut self, peer_id: ClientId) -> anyhow::Result<Option<PeerInfo>> {
        let peer = sqlx::query(
            "select bytes_transmitted, bytes_received, bytes_negotiated, first_seen, last_seen from peers where pubkey = $1",
        )
        .bind(&peer_id[..])
        .fetch_optional(&mut self.transaction)
        .await?;

        match peer {
            Some(row) => Ok(Some(PeerInfo {
                pubkey: peer_id,
                bytes_transmitted: row.try_get(0)?,
                bytes_received: row.try_get(1)?,
                bytes_negotiated: row.try_get(2)?,
                first_seen: row.try_get(3)?,
                last_seen: row.try_get(4)?,
            })),
            None => Ok(None),
        }
    }

    /// Increment peer's transmitted bytes.
    pub async fn peer_increment_transmitted(&mut self, peer_id: ClientId, amount: u64) -> anyhow::Result<()> {
        sqlx::query(
            "update peers set bytes_transmitted = bytes_transmitted + $1, last_seen = $2 where pubkey = $3",
        )
        .bind(amount as i64)
        .bind(Config::get_unix_timestamp())
        .bind(&peer_id[..])
        .execute(&mut self.transaction)
        .await?;

        Ok(())
    }

    /// Increment peer's received bytes.
    pub async fn peer_increment_received(&mut self, peer_id: ClientId, amount: u64) -> anyhow::Result<()> {
        sqlx::query(
            "update peers set bytes_received = bytes_received + $1, last_seen = $2 where pubkey = $3",
        )
        .bind(amount as i64)
        .bind(Config::get_unix_timestamp())
        .bind(&peer_id[..])
        .execute(&mut self.transaction)
        .await?;

        Ok(())
    }

    /// Get peers that have negotiated storage available.
    pub async fn find_peers_with_storage(&mut self) -> anyhow::Result<Vec<ClientId>> {
        let rows = sqlx::query(
            "select pubkey, (bytes_negotiated - bytes_transmitted) as free_storage \
             from peers where free_storage > 0 or abs(free_storage) < $1 \
             order by free_storage desc",
        )
        .bind(PEER_STORAGE_USAGE_SPREAD)
        .fetch_all(&mut self.transaction)
        .await?;

        let mut peers: Vec<ClientId> = Vec::new();
        for row in rows {
            let peer_id: &[u8] = row.try_get(0)?;
            peers.push(peer_id.try_into()?);
        }

        Ok(peers)
    }

    /// Get all peers with, optionally, last seen time within the given limit.
    pub async fn get_peers(&mut self, last_seen_limit_seconds: Option<u64>) -> anyhow::Result<Vec<PeerInfo>> {
        let oldest_timestamp = match last_seen_limit_seconds {
            Some(seconds) => Config::get_unix_timestamp() - (seconds as i64),
            None => 0,
        };

        let rows = sqlx::query(
            "select pubkey, bytes_transmitted, bytes_received, bytes_negotiated, first_seen, last_seen \
             from peers where last_seen > $1 \
             order by last_seen desc",
        )
        .bind(oldest_timestamp)
        .fetch_all(&mut self.transaction)
        .await?;

        let mut peers: Vec<PeerInfo> = Vec::new();
        for row in rows {
            let pubkey: &[u8] = row.try_get(0)?;

            peers.push(PeerInfo {
                pubkey: pubkey.try_into()?,
                bytes_transmitted: row.try_get(1)?,
                bytes_received: row.try_get(2)?,
                bytes_negotiated: row.try_get(3)?,
                first_seen: row.try_get(4)?,
                last_seen: row.try_get(5)?,
            });
        }

        Ok(peers)
    }

    /// Update peer's last seen time.
    pub async fn peer_update_last_seen(&mut self, peer_id: ClientId) -> anyhow::Result<()> {
        sqlx::query("update peers set last_seen = $1 where pubkey = $2")
            .bind(Config::get_unix_timestamp())
            .bind(&peer_id[..])
            .execute(&mut self.transaction)
            .await?;

        Ok(())
    }
}
