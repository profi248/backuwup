//! Write received peer backup files to disk.

use std::{fs, path::PathBuf};

use anyhow::bail;
use shared::types::{ClientId, PackfileId, TransportSessionNonce};
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use crate::{
    backup::filesystem::file_utils::{get_index_path, get_packfile_path},
    config::peers::PeerInfo,
    defaults::{INDEX_FOLDER, PACKFILE_FOLDER, PEER_STORAGE_USAGE_SPREAD},
    net_p2p::{obfuscate_data_impl, receive, receive::Receiver},
    CONFIG,
};

pub struct PeerDataReceiver {
    file_path: PathBuf,
    peer_id: ClientId,
    obfuscation_key: [u8; 4],
}

#[async_trait::async_trait]
impl Receiver for PeerDataReceiver {
    async fn save_index(&self, id: u32, data: &mut [u8]) -> anyhow::Result<()> {
        let path = get_index_path(&self.file_path, id);
        self.save_file(path, data).await
    }

    async fn save_packfile(&self, id: PackfileId, data: &mut [u8]) -> anyhow::Result<()> {
        let path = get_packfile_path(&self.file_path, id, true)?;
        self.save_file(path, data).await
    }
}

impl PeerDataReceiver {
    pub async fn new(peer_id: ClientId) -> anyhow::Result<Self> {
        let config = CONFIG.get().unwrap();
        let mut file_path = config.get_received_packfiles_folder()?;
        file_path.push(hex::encode(peer_id));

        let obfuscation_key = config.get_obfuscation_key().await?.to_le_bytes();

        fs::create_dir_all(&file_path)?;
        fs::create_dir_all(file_path.join(INDEX_FOLDER))?;
        fs::create_dir_all(file_path.join(PACKFILE_FOLDER))?;

        Ok(Self { file_path, peer_id, obfuscation_key })
    }

    pub async fn save_file(&self, path: PathBuf, data: &mut [u8]) -> anyhow::Result<()> {
        if path.try_exists()? {
            bail!("file name collision at path {path:?}")
        }

        let config = CONFIG.get().unwrap();
        match config.get_peer_info(self.peer_id).await? {
            Some(peer) if is_peer_allowed_to_send_data(&peer) => {
                fs::write(path, self.obfuscate_data(data))?;

                config
                    .peer_increment_received(self.peer_id, data.len() as u64)
                    .await?;
                Ok(())
            }
            Some(_) => {
                bail!("peer {} is not allowed to send more files", hex::encode(self.peer_id))
            }
            None => bail!("peer {} not found when receiving a file", hex::encode(self.peer_id)),
        }
    }

    pub fn obfuscate_data<'a>(&self, data: &'a mut [u8]) -> &'a [u8] {
        obfuscate_data_impl(data, self.obfuscation_key)
    }
}

pub async fn handle_receiving(
    client_id: ClientId,
    nonce: TransportSessionNonce,
    stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
) -> anyhow::Result<()> {
    let peer = CONFIG.get().unwrap().get_peer_info(client_id).await?;

    match peer {
        Some(peer) if is_peer_allowed_to_send_data(&peer) => {
            let receiver = PeerDataReceiver::new(client_id).await?;
            receive::handle_stream(stream, nonce, client_id, receiver).await?;
            Ok(())
        }
        Some(_) => bail!("peer {} is not allowed to send more packfiles", hex::encode(client_id)),
        None => bail!("ignoring a request from an unknown peer {}", hex::encode(client_id)),
    }
}

pub fn is_peer_allowed_to_send_data(peer: &PeerInfo) -> bool {
    peer.bytes_negotiated - peer.bytes_received > 0
        || peer.bytes_negotiated.abs_diff(peer.bytes_received) < PEER_STORAGE_USAGE_SPREAD
}
