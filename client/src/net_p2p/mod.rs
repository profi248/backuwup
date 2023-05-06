//! Contains the P2P networking code.

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

/// Returns the WebSocket configuration used for P2P connections.
fn get_ws_config() -> WebSocketConfig {
    WebSocketConfig {
        max_send_queue: None,
        max_message_size: Some(MAX_ENCAPSULATED_BACKUP_CHUNK_SIZE),
        max_frame_size: Some(MAX_ENCAPSULATED_BACKUP_CHUNK_SIZE),
        accept_unmasked_frames: false,
    }
}

/// Returns the local IP address and a random high range port, used for listening for peer connections.
pub fn get_listener_address() -> anyhow::Result<(String, u16)> {
    // get our IP address on the local network, since we can currently only connect to
    // computers in the same network segment
    let local_ip_addr = local_ip_address::local_ip()?;

    // try picking an unused high range port
    let port = pick_unused_port().ok_or(anyhow!("Unable to pick an unused port"))?;

    Ok((format!("{local_ip_addr}:{port}"), port))
}

/// Obfuscates/deobfuscates the given data in-place using the given key.
pub fn obfuscate_data_impl(data: &mut [u8], key: [u8; 4]) -> &[u8] {
    for dword in &mut data.chunks_mut(4) {
        // obfuscate each byte of the dword, the length of the chunk will be at most 4, but the last one may be shorter
        for (idx, byte) in dword.iter_mut().enumerate() {
            *byte ^= key[idx];
        }
    }

    data
}

#[test]
fn obfuscation_test() {
    use rand_chacha::{
        rand_core::{RngCore, SeedableRng},
        ChaCha8Rng,
    };

    let mut data: [u8; 123_123] = [0; 123_123];
    ChaCha8Rng::from_seed([0; 32]).fill_bytes(&mut data);
    let orig = data;
    let key = [0x40, 0x41, 0x42, 0x43];

    obfuscate_data_impl(&mut data, key);
    obfuscate_data_impl(&mut data, key);

    assert_eq!(orig, data);
}
