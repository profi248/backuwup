use std::{fs, path::PathBuf};

use anyhow::bail;
use shared::{
    server_message_ws::IncomingTransportRequest,
    types::{ClientId, PackfileHash},
};

use crate::{net_p2p::receive, net_server::requests, CONFIG};

pub struct Receiver {
    file_path: PathBuf,
    peer_id: ClientId,
}

// todo validate that the peer is allowed to send us packfiles, and increment sizes
// todo obfuscate data before saving it to disk
impl Receiver {
    pub async fn new(peer_id: ClientId) -> anyhow::Result<Self> {
        let mut file_path = CONFIG.get().unwrap().get_received_packfiles_folder()?;
        file_path.push(hex::encode(peer_id));

        fs::create_dir_all(&file_path)?;

        Ok(Self { file_path, peer_id })
    }

    pub async fn save_packfile(&self, hash: PackfileHash, data: Vec<u8>) -> anyhow::Result<()> {
        let mut path = self.file_path.clone();
        path.push(hex::encode(hash));

        if path.try_exists()? {
            bail!("Packfile hash collision");
        }

        fs::write(path, data)?;

        Ok(())
    }
}

pub async fn receive_request(request: IncomingTransportRequest) -> anyhow::Result<()> {
    let receiver = Receiver::new(request.source_client_id).await?;

    let (addr, port) = receive::get_listener_address()?;
    requests::backup_transport_confirm(request.source_client_id, addr).await?;

    tokio::spawn(receive::listen(port, request.session_nonce, request.source_client_id, receiver));

    Ok(())
}
