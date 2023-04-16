use std::{fs, path::PathBuf};

use anyhow::bail;
use shared::types::{ClientId, PackfileId, TransportSessionNonce};
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use crate::{
    config::peers::PeerInfo,
    defaults::{INDEX_FOLDER, PACKFILE_FOLDER, PEER_STORAGE_USAGE_SPREAD},
    net_p2p::receive,
    net_server::requests,
    CONFIG,
};

pub struct Receiver {
    file_path: PathBuf,
    peer_id: ClientId,
}

impl Receiver {
    pub async fn new(peer_id: ClientId) -> anyhow::Result<Self> {
        let mut file_path = CONFIG.get().unwrap().get_received_packfiles_folder()?;
        file_path.push(hex::encode(peer_id));

        fs::create_dir_all(&file_path)?;
        fs::create_dir_all(file_path.join(INDEX_FOLDER))?;
        fs::create_dir_all(file_path.join(PACKFILE_FOLDER))?;

        Ok(Self { file_path, peer_id })
    }

    pub async fn save_file(&self, path: PathBuf, data: &mut [u8]) -> anyhow::Result<()> {
        if path.try_exists()? {
            bail!("file name collision at path {path:?}")
        }

        let config = CONFIG.get().unwrap();
        match config.get_peer_info(self.peer_id).await? {
            Some(peer) if is_peer_allowed_to_send_data(&peer) => {
                fs::write(path, obfuscate_data(data))?;

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

    pub async fn save_index(&self, id: u32, data: &mut [u8]) -> anyhow::Result<()> {
        let path = self.file_path.join(INDEX_FOLDER).join(format!("{:0>10}", id));

        self.save_file(path, data).await
    }

    pub async fn save_packfile(&self, id: PackfileId, data: &mut [u8]) -> anyhow::Result<()> {
        let path = self.get_packfile_path(id)?;

        self.save_file(path, data).await
    }

    pub fn get_packfile_path(&self, id: PackfileId) -> anyhow::Result<PathBuf> {
        let mut path = self.file_path.clone().join(PACKFILE_FOLDER);
        let hex = hex::encode(id);

        // save the packfile in a folder named after the first two bytes of the hash
        path.push(&hex[..2]);
        fs::create_dir_all(&path)?;

        path.push(hex);
        Ok(path)
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
            let receiver = Receiver::new(client_id).await?;
            receive::receive_handle_stream(stream, nonce, client_id, receiver).await?;
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

// todo take an actual random key instead of a hardcoded one
pub fn obfuscate_data(data: &mut [u8]) -> &[u8] {
    // for byte in data.iter_mut() {
    //     *byte ^= 0x42;
    // }
    data
}
