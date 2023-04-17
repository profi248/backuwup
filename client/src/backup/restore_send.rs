use anyhow::bail;
use shared::{
    p2p_message::FileInfo,
    types::{ClientId, TransportSessionNonce},
};
use tokio::{fs, net::TcpStream};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use crate::{
    backup::filesystem::file_utils::{parse_index_path_into_id, parse_packfile_path_into_id},
    defaults::{INDEX_FOLDER, PACKFILE_FOLDER},
    log,
    net_p2p::transport::BackupTransportManager,
    CONFIG,
};

pub async fn restore_all_data_to_peer(
    peer_id: ClientId,
    nonce: TransportSessionNonce,
    socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
) -> anyhow::Result<()> {
    let mut transport = BackupTransportManager::new(socket, nonce, peer_id);
    let mut file_path = CONFIG.get().unwrap().get_received_packfiles_folder()?;
    file_path.push(hex::encode(peer_id));

    log!("[restore] restoring packfiles to peer");
    for entry in file_path.join(PACKFILE_FOLDER).read_dir()? {
        match entry {
            Ok(entry) if entry.file_type()?.is_dir() => {
                for packfile in entry.path().read_dir()? {
                    match packfile {
                        Ok(packfile) if packfile.file_type()?.is_file() => {
                            let data = fs::read(packfile.path()).await?;
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

    log!("[restore] restoring index to peer");
    for entry in file_path.join(INDEX_FOLDER).read_dir()? {
        match entry {
            Ok(entry) if entry.file_type()?.is_file() => {
                let data = fs::read(entry.path()).await?;
                let index_id = parse_index_path_into_id(&entry.path())?;
                transport.send_data(data, FileInfo::Index(index_id)).await?;
                println!("sending index: {index_id:?}");
            }
            // skip any other entries if present
            Ok(_) => continue,
            Err(e) => bail!("error reading received index director: {e}"),
        }
    }

    log!("[restore] restoring done!");
    Ok(())
}
