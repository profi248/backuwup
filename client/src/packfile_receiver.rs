use std::{fs, path::PathBuf};

use anyhow::bail;
use shared::{
    server_message_ws::IncomingTransportRequest,
    types::{ClientId, PackfileId},
};

use crate::{
    config::peers::PeerInfo, defaults::PEER_STORAGE_USAGE_SPREAD, net_p2p::receive,
    net_server::requests, CONFIG,
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

        Ok(Self { file_path, peer_id })
    }

    pub async fn save_packfile(&self, hash: PackfileId, data: &mut [u8]) -> anyhow::Result<()> {
        let path = self.get_packfile_path(hash)?;

        if path.try_exists()? {
            bail!("Packfile ID collision");
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
                bail!("Peer {} is not allowed to send more packfiles", hex::encode(self.peer_id))
            }
            None => bail!("Peer {} not found when receiving a packfile", hex::encode(self.peer_id)),
        }
    }

    pub fn get_packfile_path(&self, id: PackfileId) -> anyhow::Result<PathBuf> {
        let mut path = self.file_path.clone();
        let hex = hex::encode(id);

        // save the packfile in a folder named after the first two bytes of the hash
        path.push(&hex[..2]);
        fs::create_dir_all(&path)?;

        path.push(hex);
        Ok(path)
    }
}

pub async fn receive_request(request: IncomingTransportRequest) -> anyhow::Result<()> {
    let peer = CONFIG
        .get()
        .unwrap()
        .get_peer_info(request.source_client_id)
        .await?;

    match peer {
        Some(peer) if is_peer_allowed_to_send_data(&peer) => {
            let receiver = Receiver::new(request.source_client_id).await?;

            let (addr, port) = receive::get_listener_address()?;
            requests::backup_transport_confirm(request.source_client_id, addr).await?;

            tokio::spawn(receive::listen(
                port,
                request.session_nonce,
                request.source_client_id,
                receiver,
            ));
            Ok(())
        }
        Some(_) => bail!(
            "Peer {} is not allowed to send more packfiles",
            hex::encode(request.source_client_id)
        ),
        None => bail!(
            "Received a transport request from an unknown peer {}, ignoring",
            hex::encode(request.source_client_id)
        ),
    }
}

pub fn is_peer_allowed_to_send_data(peer: &PeerInfo) -> bool {
    (peer.bytes_negotiated as i64 - peer.bytes_received as i64) > 0
        || peer.bytes_negotiated.abs_diff(peer.bytes_received) < PEER_STORAGE_USAGE_SPREAD
}

// todo take an actual random key instead of a hardcoded one
pub fn obfuscate_data(data: &mut [u8]) -> &[u8] {
    // for byte in data.iter_mut() {
    //     *byte ^= 0x42;
    // }
    data
}
