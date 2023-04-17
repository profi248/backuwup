use std::{fs, path::PathBuf};

use shared::types::{ClientId, PackfileId, TransportSessionNonce};
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use crate::{
    backup::filesystem::file_utils::{get_index_path, get_packfile_path},
    config::peers::PeerInfo,
    defaults::{INDEX_FOLDER, PACKFILE_FOLDER, PEER_STORAGE_USAGE_SPREAD},
    net_p2p::{receive, receive::Receiver},
    CONFIG,
};

pub struct RestoreReceiver {
    file_path: PathBuf,
    peer_id: ClientId,
}

#[async_trait::async_trait]
impl Receiver for RestoreReceiver {
    async fn save_index(&self, id: u32, data: &mut [u8]) -> anyhow::Result<()> {
        let path = get_index_path(&self.file_path, id);
        self.save_file(path, data).await
    }

    async fn save_packfile(&self, id: PackfileId, data: &mut [u8]) -> anyhow::Result<()> {
        let path = get_packfile_path(&self.file_path, id, true)?;
        self.save_file(path, data).await
    }
}

// todo throttle restore requests to prevent a DoS
impl RestoreReceiver {
    pub async fn new(peer_id: ClientId) -> anyhow::Result<Self> {
        let file_path = CONFIG.get().unwrap().get_restored_packfiles_folder()?;

        fs::create_dir_all(&file_path)?;
        fs::create_dir_all(file_path.join(INDEX_FOLDER))?;
        fs::create_dir_all(file_path.join(PACKFILE_FOLDER))?;

        Ok(Self { file_path, peer_id })
    }

    pub async fn save_file(&self, path: PathBuf, data: &mut [u8]) -> anyhow::Result<()> {
        fs::write(path, deobfuscate_data(data))?;

        Ok(())
    }
}

pub async fn handle_receiving(
    client_id: ClientId,
    nonce: TransportSessionNonce,
    stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
) -> anyhow::Result<()> {
    let receiver = RestoreReceiver::new(client_id).await?;
    receive::receive_handle_stream(stream, nonce, client_id, receiver).await?;

    Ok(())
}

// todo take an actual random key instead of a hardcoded one
pub fn deobfuscate_data(data: &mut [u8]) -> &[u8] {
    // for byte in data.iter_mut() {
    //     *byte ^= 0x42;
    // }
    data
}
