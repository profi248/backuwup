use anyhow::anyhow;
use portpicker::pick_unused_port;
use shared::p2p_message::MAX_ENCAPSULATED_BACKUP_CHUNK_SIZE;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;

pub mod handle_connections;
pub mod p2p_connection_manager;
pub mod receive;
pub mod received_files_writer;
pub mod restore_files_writer;
pub mod transport;

fn get_ws_config() -> WebSocketConfig {
    WebSocketConfig {
        max_send_queue: None,
        max_message_size: Some(MAX_ENCAPSULATED_BACKUP_CHUNK_SIZE),
        max_frame_size: Some(MAX_ENCAPSULATED_BACKUP_CHUNK_SIZE),
        accept_unmasked_frames: false,
    }
}

pub fn get_listener_address() -> anyhow::Result<(String, u16)> {
    // get our IP address on the local network, since we can currently only connect to
    // computers in the same network segment
    let local_ip_addr = local_ip_address::local_ip()?;

    // try picking an unused high range port
    let port = pick_unused_port().ok_or(anyhow!("Unable to pick an unused port"))?;

    Ok((format!("{local_ip_addr}:{port}"), port))
}
