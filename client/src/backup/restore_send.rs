//! Contains logic for sending all received data to a peer during a restore operation.

use anyhow::bail;
use shared::{
    p2p_message::FileInfo,
    types::{ClientId, TransportSessionNonce},
};
use tokio::{fs, net::TcpStream};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use crate::{
    backup::filesystem::file_utils::{parse_index_path_into_id, parse_packfile_path_into_id},
    config::Config,
    defaults::{INDEX_FOLDER, PACKFILE_FOLDER, RESTORE_THROTTLE_DELAY},
    log,
    net_p2p::{obfuscate_data_impl, transport::BackupTransportManager},
    CONFIG,
};

/// Send all packfiles and index files that we have received from a peer to that peer, over an
/// already established WebSocket connection.
pub async fn restore_all_data_to_peer(
    peer_id: ClientId,
    nonce: TransportSessionNonce,
    socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
) -> anyhow::Result<()> {
    let config = CONFIG.get().unwrap();

    if let Some(last_request) = config.get_last_peer_restore_request(peer_id).await? {
        let remaining = (RESTORE_THROTTLE_DELAY as i64) - (Config::get_unix_timestamp() - last_request);
        if last_request > Config::get_unix_timestamp() - (RESTORE_THROTTLE_DELAY as i64) {
            bail!("rate limited restore request from {}, wait {remaining}s", hex::encode(peer_id));
        }
    }

    config.log_peer_restore_request(peer_id).await?;

    let mut transport = BackupTransportManager::new(socket, nonce, peer_id);
    let mut file_path = CONFIG.get().unwrap().get_received_packfiles_folder()?;
    file_path.push(hex::encode(peer_id));

    let obfuscation_key = CONFIG.get().unwrap().get_obfuscation_key().await?.to_le_bytes();

    log!("[rsend] restoring packfiles to peer");
    for entry in file_path.join(PACKFILE_FOLDER).read_dir()? {
        match entry {
            Ok(entry) if entry.file_type()?.is_dir() => {
                for packfile in entry.path().read_dir()? {
                    match packfile {
                        Ok(packfile) if packfile.file_type()?.is_file() => {
                            let mut data = fs::read(packfile.path()).await?;

                            // deobfuscate data on disk using the same algorithm as obfuscation as it's a simple xor
                            obfuscate_data_impl(&mut data, obfuscation_key);

                            let packfile_id = parse_packfile_path_into_id(&packfile.path())?;
                            transport.send_data(data, FileInfo::Packfile(packfile_id)).await?;
                            println!("sending packfile: {packfile_id:?}");
                        }
                        Ok(_) => continue,
                        Err(e) => bail!("error reading received packfile: {e}"),
                    }
                }
            }
            // skip any other entries if present
            Ok(_) => continue,
            Err(e) => bail!("error reading received packfile directory: {e}"),
        }
    }

    log!("[rsend] restoring index to peer");
    for entry in file_path.join(INDEX_FOLDER).read_dir()? {
        match entry {
            Ok(entry) if entry.file_type()?.is_file() => {
                let mut data = fs::read(entry.path()).await?;

                // deobfuscate data on disk using the same algorithm as obfuscation as it's a simple xor
                obfuscate_data_impl(&mut data, obfuscation_key);

                let index_id = parse_index_path_into_id(&entry.path())?;

                transport.send_data(data, FileInfo::Index(index_id)).await?;
                println!("sending index: {index_id:?}");
            }
            // skip any other entries if present
            Ok(_) => continue,
            Err(e) => bail!("error reading received index director: {e}"),
        }
    }

    transport.done().await;
    log!("[rsend] restore sending done!");
    Ok(())
}
