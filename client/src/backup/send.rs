use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, bail};
use fs_extra::dir::get_size;
use shared::{
    p2p_message::{FileInfo, RequestType},
    server_message_ws::BackupMatched,
    types::{ClientId, PackfileId, TransportSessionNonce},
};
use tokio::{net::TcpStream, time};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use crate::{
    backup::{filesystem::file_utils, BACKUP_ORCHESTRATOR},
    defaults::{
        INDEX_FOLDER, MAX_PACKFILE_LOCAL_BUFFER_SIZE, PACKFILE_FOLDER,
        PACKFILE_LOCAL_BUFFER_RESUME_THRESHOLD, STORAGE_REQUEST_RETRY_DELAY,
    },
    log,
    net_p2p::transport::BackupTransportManager,
    net_server::{requests, requests::p2p_connection_begin},
    CONFIG, TRANSPORT_REQUESTS, UI,
};

pub async fn send(output_folder: PathBuf) -> anyhow::Result<()> {
    let orchestrator = BACKUP_ORCHESTRATOR.get().unwrap();

    let pack_folder = output_folder.join(PACKFILE_FOLDER);
    let index_folder = output_folder.join(INDEX_FOLDER);

    let mut last_written = orchestrator.packfile_bytes_written();
    let mut last_matched = orchestrator.get_storage_request_last_matched();
    let mut connection = None;
    let mut packfiles_done = false;
    let mut index_done = false;

    // sending loop that takes care of reestablishing the connection and transporting all files
    loop {
        // todo flushing the packer here would be useful
        // temporarily stop the backup if the local buffer got too large
        if orchestrator.available_packfile_bytes() > MAX_PACKFILE_LOCAL_BUFFER_SIZE {
            orchestrator.pause().await;
        }

        let current_written = orchestrator.packfile_bytes_written();
        let current_matched = orchestrator.get_storage_request_last_matched();

        // try establishing a peer connection if we just started or connection got terminated
        if connection.is_none() {
            match get_peer_connection().await {
                Ok((peer_id, transport)) => {
                    UI.get().unwrap().progress_add_peer(peer_id).await;
                    log!("[send] connection established with {}", hex::encode(peer_id));
                    connection = Some((peer_id, transport));
                }
                Err(e) => {
                    log!("[send] unable to get a peer connection: {}", e);
                    connection = None;
                }
            }
        }

        println!("current_written: {current_written} last_written: {last_written} current_matched: {current_matched} last_matched: {last_matched}");
        println!(
            "current_written > last_written: {}, current_matched > last_matched: {}",
            current_written > last_written,
            current_matched > last_matched
        );

        println!("is_packing_completed: {}", orchestrator.is_packing_completed());
        println!("available_packfile_bytes: {} B", orchestrator.available_packfile_bytes());

        if let Some(conn) = &mut connection {
            if ((current_written > last_written || current_matched > last_matched)
                || orchestrator.is_packing_completed())
                && !packfiles_done
            {
                let send_result = send_packfiles_from_folder(&pack_folder, conn.0, &mut conn.1).await;

                // todo distinguish between send errors and filesystem errors
                match send_result {
                    Ok(_) => {
                        last_matched = current_matched;
                        last_written = current_written;
                    }
                    Err(e) => {
                        log!("[send] error sending packfiles: {}", e);
                        connection = None;
                        continue;
                    }
                }

                // resume the backup if we get enough packfiles (over constant threshold) to send
                if (MAX_PACKFILE_LOCAL_BUFFER_SIZE - orchestrator.available_packfile_bytes())
                    > PACKFILE_LOCAL_BUFFER_RESUME_THRESHOLD
                    && !orchestrator.should_continue()
                {
                    orchestrator.resume().await;
                }

                // if packing is completed and all packfiles have been sent, we are done
                if orchestrator.is_packing_completed() && get_size(&pack_folder)? == 0 {
                    log!("[send] packfile sending done, will send index files");
                    packfiles_done = true;
                }
            }

            if packfiles_done && !index_done {
                let send_result = send_index(&index_folder, conn.0, &mut conn.1).await;
                match send_result {
                    Ok(_) => index_done = true,
                    Err(e) => {
                        log!("[send] error sending index files: {e}");
                        connection = None;
                        continue;
                    }
                }
            }

            if packfiles_done && index_done {
                break;
            }
        }

        // wait for an arbitrary amount of time until the next check
        time::sleep(Duration::from_secs(1)).await;
    }

    log!("[send] sending done!");
    Ok(())
}

async fn send_index(
    folder: &Path,
    peer_id: ClientId,
    transport: &mut BackupTransportManager,
) -> anyhow::Result<()> {
    let config = CONFIG.get().unwrap();
    for file in folder.read_dir()? {
        match file {
            Ok(file) if file.file_type()?.is_file() => {
                let path = file.path();
                let index_num = file_utils::parse_index_path_into_id(&path)?;

                // skip the index files that have been already sent
                let highest_sent_index = config.get_highest_sent_index_number().await?;
                if highest_sent_index.is_some() && index_num <= highest_sent_index.unwrap() {
                    continue;
                }

                println!("[send] sending index file {}", path.display());

                // send the index file, and don't delete it, we will need it for deduplication
                if transport
                    .send_data(fs::read(&path)?, FileInfo::Index(index_num))
                    .await
                    .is_ok()
                {
                    config
                        .peer_increment_transmitted(peer_id, fs::metadata(&path)?.len())
                        .await?;
                    config.save_highest_sent_index_number(index_num).await?;
                    println!("[send] index file {} sent successfully", path.display());
                } else {
                    bail!("[send] sending index file {} failed", path.display());
                }
            }
            Ok(_) => {} // ignore anything else
            Err(e) => bail!("cannot read when sending index files: {e}"),
        }
    }

    Ok(())
}

async fn send_packfiles_from_folder(
    folder: &Path,
    peer_id: ClientId,
    transport: &mut BackupTransportManager,
) -> anyhow::Result<()> {
    for packfile in folder.read_dir()? {
        match packfile {
            Ok(entry) if entry.file_type()?.is_dir() => {
                // maybe we can parallelize this later
                for packfile in entry.path().read_dir()? {
                    match packfile {
                        Ok(entry) if entry.file_type()?.is_file() => {
                            send_single_packfile(&entry.path(), peer_id, transport).await?;
                        }
                        Err(e) => bail!("Error reading a packfile when sending: {e}"),
                        Ok(_) => {} // ignore folders
                    }
                }
            }
            Err(e) => bail!("Error reading from packfiles folder when sending: {e}"),
            Ok(_) => {} // we expect a specific folder structure so ignore everything else
        }
    }

    Ok(())
}

async fn get_peer_connection() -> anyhow::Result<(ClientId, BackupTransportManager)> {
    let orchestrator = BACKUP_ORCHESTRATOR.get().unwrap();
    let config = CONFIG.get().unwrap();

    let peers_with_storage = &config.find_peers_with_storage().await?;

    // first try whether we have any active connections with peers that we can send to,
    // and return the one for the peer with the most storage
    for peer in peers_with_storage {
        if let Some(transport) = orchestrator.active_transport_sessions.lock().await.remove(peer) {
            return Ok((*peer, transport));
        }
    }

    // if no connections are active, try establishing them,
    // starting with an existing peer with most storage
    for peer in peers_with_storage {
        let nonce = TRANSPORT_REQUESTS
            .get()
            .unwrap()
            .add_request(*peer, RequestType::Transport)
            .await?;

        log!("[send] trying to establish connection with {}", hex::encode(peer));
        // the client we tried to notify might not be connected to the server at all, then we skip it
        if !p2p_connection_begin(*peer, nonce).await? {
            continue;
        }

        // wait for a while for the connection to establish
        // todo ideally replace by a channel subscription
        tokio::time::sleep(Duration::from_secs(3)).await;
        if let Some(transport) = orchestrator.active_transport_sessions.lock().await.remove(peer) {
            return Ok((*peer, transport));
        }
    }

    // if we can't establish a connection with either peer or we don't have any peers with storage
    // yet, send a storage request if needed and try waiting for a bit whether it gets fulfilled now
    log!("[send] no available peers, will send storage request if needed");
    send_storage_request_if_needed().await?;
    tokio::time::sleep(Duration::from_secs(5)).await;

    // if we get a request fulfilled, the connection will be established automatically
    for peer in &config.find_peers_with_storage().await? {
        log!("[send] storage request fulfilled immediately");
        if let Some(transport) = orchestrator.active_transport_sessions.lock().await.remove(peer) {
            return Ok((*peer, transport));
        }
    }

    // if we don't get any connections now, we will have to wait
    Err(anyhow!("Unable to get any connections at this time"))
}

async fn send_single_packfile(
    path: &PathBuf,
    peer_id: ClientId,
    transport: &mut BackupTransportManager,
) -> anyhow::Result<()> {
    let orchestrator = BACKUP_ORCHESTRATOR.get().unwrap();
    let config = CONFIG.get().unwrap();

    let size = fs::metadata(path)?.len();
    println!("[send] sending packfile {}", path.display());

    // this function will wait for an acknowledgement from the other party and only return after
    // the transport is confirmed, so we should be able to safely delete the packfile
    if transport
        .send_data(fs::read(path)?, FileInfo::Packfile(file_utils::parse_packfile_path_into_id(path)?))
        .await
        .is_ok()
    {
        orchestrator.increment_packfile_bytes_sent(size);
        config.peer_increment_transmitted(peer_id, size).await?;

        fs::remove_file(path)?;

        println!("[send] packfile {} sent successfully, deleted", path.display());
        return Ok(());
    }

    Err(anyhow!("Packfile not sent"))
}

async fn send_storage_request_if_needed() -> anyhow::Result<()> {
    let orchestrator = BACKUP_ORCHESTRATOR.get().unwrap();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

    if now - orchestrator.get_storage_request_last_sent() > STORAGE_REQUEST_RETRY_DELAY {
        let request_size = estimate_storage_request_size();
        log!("[send] sending a new storage request of size {} B", request_size);

        requests::backup_storage_request(request_size).await?;
        orchestrator.update_storage_request_last_sent();
    }

    Ok(())
}

// todo maybe store to which peers we have sent a packfile
pub async fn handle_storage_request_matched(matched: BackupMatched) -> anyhow::Result<()> {
    let nonce = TRANSPORT_REQUESTS
        .get()
        .unwrap()
        .add_request(matched.destination_id, RequestType::Transport)
        .await?;

    BACKUP_ORCHESTRATOR
        .get()
        .ok_or(anyhow!("Backup orchestrator not initialized"))?
        .update_storage_request_last_matched();

    CONFIG
        .get()
        .unwrap()
        .add_or_increment_peer_storage(matched.destination_id, matched.storage_available)
        .await?;

    p2p_connection_begin(matched.destination_id, nonce).await?;

    Ok(())
}

pub async fn connection_established(
    client_id: ClientId,
    nonce: TransportSessionNonce,
    socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
) -> anyhow::Result<()> {
    let transport = BackupTransportManager::new(socket, nonce, client_id);

    BACKUP_ORCHESTRATOR
        .get()
        .ok_or(anyhow!("Backup orchestrator not initialized"))?
        .active_transport_sessions
        .lock()
        .await
        .insert(client_id, transport);

    CONFIG.get().unwrap().peer_update_last_seen(client_id).await?;

    Ok(())
}

// todo do a better estimation, for example if we are just running a backup that has little changes
fn estimate_storage_request_size() -> u64 {
    BACKUP_ORCHESTRATOR.get().unwrap().get_size_estimate()
}
