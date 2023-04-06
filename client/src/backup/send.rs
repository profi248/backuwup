use std::{
    cmp::min,
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, bail};
use fs_extra::dir::get_size;
use shared::{
    constants::BACKUP_REQUEST_EXPIRY,
    server_message_ws::{BackupMatched, FinalizeTransportRequest},
};
use tokio::time;

use crate::{
    backup::BACKUP_ORCHESTRATOR, defaults::PACKFILE_FOLDER, net_server::requests,
    TRANSPORT_REQUESTS, UI,
};

pub async fn send(output_folder: PathBuf) -> anyhow::Result<()> {
    let orchestrator = BACKUP_ORCHESTRATOR.get().unwrap();

    // take only the packfile folder, index will be sent separately
    let pack_folder = output_folder.join(PACKFILE_FOLDER);

    let mut last_written = orchestrator.packfile_bytes_written();
    let mut last_matched = orchestrator.get_storage_request_last_matched();

    // todo better retries and a lot more
    loop {
        let current_written = orchestrator.packfile_bytes_written();
        let current_matched = orchestrator.get_storage_request_last_matched();

        if current_written > last_written || current_matched > last_matched {
            match send_packfiles_from_folder(&pack_folder).await {
                Ok(_) => {
                    last_matched = current_matched;
                    last_written = current_written;
                }
                Err(e) => {
                    UI.get().unwrap().log(format!("Error sending packfiles: {e}"));
                }
            }
        }

        if orchestrator.is_packing_completed() {
            // send any possible remaining packfiles
            send_packfiles_from_folder(&pack_folder).await?;
            // if no more packfiles are being written and no more packfiles are left to send, we're done
            if get_size(&pack_folder)? == 0 {
                break;
            }
        }

        time::sleep(time::Duration::from_secs(1)).await;
    }

    // todo: send index

    Ok(())
}

async fn send_packfiles_from_folder(folder: &PathBuf) -> anyhow::Result<()> {
    for packfile in folder.read_dir()? {
        match packfile {
            Ok(entry) if entry.file_type()?.is_dir() => {
                for packfile in entry.path().read_dir()? {
                    match packfile {
                        Ok(entry) if entry.file_type()?.is_file() => {
                            send_single_packfile(&entry.path()).await?
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

async fn send_single_packfile(path: &PathBuf) -> anyhow::Result<()> {
    let orchestrator = BACKUP_ORCHESTRATOR.get().unwrap();

    if orchestrator.active_transport_sessions.lock().await.is_empty() {
        send_storage_request_if_needed(path).await?;
        bail!("No active transport sessions");
    }

    let size = fs::metadata(path)?.len();

    for (idx, session) in orchestrator
        .active_transport_sessions
        .lock()
        .await
        .iter_mut()
        .enumerate()
    {
        UI.get()
            .unwrap()
            .log(format!("Sending packfile {}", path.display()));

        // todo currently we don't really know if the packfile was sent successfully, acknowledgment is probably needed
        // we also need to know how much data can we send to that peer
        if session.send_data(fs::read(path)?, [0; 12]).await.is_ok() {
            fs::remove_file(path)?;
            orchestrator.increment_packfile_bytes_sent(size);
            UI.get()
                .unwrap()
                .log(format!("Packfile {} sent successfully, deleting", path.display()));
            return Ok(());
        } else {
            orchestrator
                .active_transport_sessions
                .lock()
                .await
                .swap_remove(idx)
                .done()
                .await;
            continue;
        }
    }

    Err(anyhow!("Packfile not sent"))
}

async fn send_storage_request_if_needed(path: &PathBuf) -> anyhow::Result<()> {
    let orchestrator = BACKUP_ORCHESTRATOR.get().unwrap();
    let logger = UI.get().unwrap();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

    if now - orchestrator.get_storage_request_last_sent() > BACKUP_REQUEST_EXPIRY {
        let request_size = estimate_storage_request_size(path)?;
        logger.log(format!("Sending a new storage request of size {request_size} B"));

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
                .push(mgr);
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
    Ok(min(get_size(path)?, crate::defaults::MAX_PACKFILE_LOCAL_BUFFER_SIZE as u64))
}
