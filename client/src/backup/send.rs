use std::{
    cmp::min,
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, bail};
use fs_extra::dir::get_size;
use shared::{
    constants::BACKUP_REQUEST_EXPIRY,
    server_message_ws::{BackupMatched, FinalizeTransportRequest},
    types::{ClientId, PackfileId},
};
use tokio::time;

use crate::{
    backup::BACKUP_ORCHESTRATOR,
    defaults::PACKFILE_FOLDER,
    log,
    net_p2p::transport::BackupTransportManager,
    net_server::{requests, requests::backup_transport_begin},
    CONFIG, TRANSPORT_REQUESTS, UI,
};
use crate::defaults::{MAX_PACKFILE_LOCAL_BUFFER_SIZE, PACKFILE_LOCAL_BUFFER_RESUME_THRESHOLD};

pub async fn send(output_folder: PathBuf) -> anyhow::Result<()> {
    let orchestrator = BACKUP_ORCHESTRATOR.get().unwrap();

    // take only the packfile folder, index will be sent separately
    let pack_folder = output_folder.join(PACKFILE_FOLDER);

    let mut last_written = orchestrator.packfile_bytes_written();
    let mut last_matched = orchestrator.get_storage_request_last_matched();
    let mut connection = None;

    // todo better retries and a lot more
    loop {
        let current_written = orchestrator.packfile_bytes_written();
        let current_matched = orchestrator.get_storage_request_last_matched();

        struct Connection {
            peer_id: ClientId,
            transport: BackupTransportManager,
        }

        if connection.is_none() {
            match get_peer_connection().await {
                Ok((peer_id, transport)) => {
                    log!("[send] connection established with {}", hex::encode(peer_id));
                    connection = Some(Connection { peer_id, transport });
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
            if (current_written > last_written || current_matched > last_matched)
                || orchestrator.is_packing_completed() {
                let send_result = send_packfiles_from_folder(
                    &pack_folder,
                    conn.peer_id,
                    &mut conn.transport,
                ).await;

                match send_result {
                    Ok(_) => {
                        last_matched = current_matched;
                        last_written = current_written;
                    }
                    Err(e) => {
                        log!("[send] error sending packfiles: {}", e);
                        connection = None;
                    }
                }

                if MAX_PACKFILE_LOCAL_BUFFER_SIZE - orchestrator.available_packfile_bytes() > PACKFILE_LOCAL_BUFFER_RESUME_THRESHOLD {
                    if !orchestrator.should_continue() { orchestrator.resume().await; }
                }

                if orchestrator.is_packing_completed() && get_size(&pack_folder)? == 0 {
                    break;
                }
            }
        }

        time::sleep(Duration::from_secs(1)).await;
    }

    // todo: send index

    log!("[send] sending done!");
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
        let nonce = TRANSPORT_REQUESTS.get().unwrap().add_request(*peer).await?;

        log!("[send] trying to establish connection with {}", hex::encode(peer));
        // the client we tried to notify might not be connected to the server at all, then we skip it
        if !backup_transport_begin(*peer, nonce).await? {
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
    log!("[send] sending packfile {}", path.display());

    // parse packfile id from its name
    let packfile_id: PackfileId = hex::decode(
        path.file_name()
            .ok_or(anyhow!("can't get packfile filename"))?
            .to_string_lossy()
            .to_string(),
    )?
    .try_into()
    .map_err(|_| anyhow!("invalid packfile filename"))?;

    // this function will wait for an acknowledgement from the other party and only return after
    // the transport is confirmed, so we should be able to safely delete the packfile
    if transport.send_data(fs::read(path)?, packfile_id).await.is_ok() {
        orchestrator.increment_packfile_bytes_sent(size);
        config.peer_increment_transmitted(peer_id, size).await?;

        fs::remove_file(path)?;

        log!("[send] packfile {} sent successfully, deleted", path.display());
        return Ok(());
    }

    Err(anyhow!("Packfile not sent"))
}

async fn send_storage_request_if_needed() -> anyhow::Result<()> {
    let orchestrator = BACKUP_ORCHESTRATOR.get().unwrap();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

    if now - orchestrator.get_storage_request_last_sent() > BACKUP_REQUEST_EXPIRY {
        let request_size = estimate_storage_request_size(&orchestrator.destination_path)?;
        log!("[send] sending a new storage request of size {} B", request_size);

        requests::backup_storage_request(request_size).await?;
        orchestrator.update_storage_request_last_sent();
    }

    Ok(())
}

// todo store the storage granted to other peers in config
// todo maybe store to which peers we have sent a packfile
pub async fn handle_storage_request_matched(matched: BackupMatched) -> anyhow::Result<()> {
    let nonce = TRANSPORT_REQUESTS
        .get()
        .unwrap()
        .add_request(matched.destination_id)
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

    requests::backup_transport_begin(matched.destination_id, nonce).await?;

    Ok(())
}

pub async fn handle_finalize_transport_request(
    request: FinalizeTransportRequest,
) -> anyhow::Result<()> {
    let transport = TRANSPORT_REQUESTS
        .get()
        .unwrap()
        .finalize_request(request.destination_client_id, request.destination_ip_address)
        .await;

    match transport {
        Ok(Some(mgr)) => {
            BACKUP_ORCHESTRATOR
                .get()
                .ok_or(anyhow!("Backup orchestrator not initialized"))?
                .active_transport_sessions
                .lock()
                .await
                .insert(request.destination_client_id, mgr);
        }
        Err(e) => bail!(e),
        Ok(None) => {}
    }

    Ok(())
}

pub async fn make_backup_storage_request(storage_required: u64) -> anyhow::Result<()> {
    requests::backup_storage_request(storage_required).await?;

    Ok(())
}

fn estimate_storage_request_size(path: &PathBuf) -> anyhow::Result<u64> {
    // todo try estimating based on source backup folder if needed
    // Ok(min(get_size(path)?, crate::defaults::MAX_PACKFILE_LOCAL_BUFFER_SIZE as u64))
    Ok(200_000_000)
}
