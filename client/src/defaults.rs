//! Contains the default values for the configuration and various constants.

/// The address of the global server.
pub const SERVER_ADDR: &str = "127.0.0.1:9999";

/// Toggles whether the client uses TLS and certificate validation for communicating with the server.
pub const SERVER_USE_TLS: bool = true;

/// The address of the user interface server.
pub const UI_BIND_ADDR: &str = "127.0.0.1:3000";

/// The name of the folder that contains the application data.
pub const APP_FOLDER_NAME: &str = "backuwup";

/// The name of the configuration database.
pub const CONFIG_DB_FILE: &str = "config.db";

/// The name of folder that contains the packfiles.
pub const PACKFILE_FOLDER: &str = "pack";

/// The name of folder that contains the index files.
pub const INDEX_FOLDER: &str = "index";

/// Folder name for storing packfiles that are generated locally and are waiting to be sent to other peers.
pub const BACKUP_BUFFER_FOLDER_NAME: &str = "local_packfiles";

/// Folder name for storing packfiles received from other peers.
pub const RECEIVED_PACKFILES_FOLDER: &str = "received_packfiles";

/// Folder name for storing packfiles that received from other peers in the process of backup restoration.
pub const RESTORE_BUFFER_FOLDER: &str = "restore_packfiles";

/// Maximum storage used over the negotiated storage space with other peers, per peer.
pub const PEER_STORAGE_USAGE_SPREAD: i64 = 16 * 1024 * 1024; // 16 MiB

/// Maximum size of packfiles that are allowed to be temporarily stored on disk,
/// while waiting for transferring them to a peer.
pub const MAX_PACKFILE_LOCAL_BUFFER_SIZE: u64 = 100 * 1024 * 1024; // 100 MiB

/// Maximum amount of seconds to wait until considering packfile transfer as failed.
pub const PACKFILE_SEND_TIMEOUT: u64 = 20;

/// Maximum amount of seconds to wait until considering packfile ack as failed.
pub const PACKFILE_ACK_TIMEOUT: u64 = 5;

/// Minimum number of seconds to wait before retrying to send a storage request.
pub const STORAGE_REQUEST_RETRY_DELAY: u64 = 10;

/// The number of seconds to wait before allowing a certain peer to resend a request to restore.
pub const RESTORE_THROTTLE_DELAY: u64 = 60;

/// The maximum size of a single storage request at a time.
pub const STORAGE_REQUEST_CAP: u64 = 150_000_000; // 150 MB

/// The default size of a single storage request, if the size cannot be estimated.
pub const STORAGE_REQUEST_STEP: u64 = 50_000_000; // 50 MB

/// The amount of free space under the packfile maximum local buffer size to trigger a backup resume.
pub const PACKFILE_LOCAL_BUFFER_RESUME_THRESHOLD: u64 = 50 * 1024 * 1024;

/// Maximum size of blob data that's allowed in a packfile.
pub const BLOB_MAX_UNCOMPRESSED_SIZE: usize = 3 * 1024 * 1024; // 3 MiB

/// Minimum size of blob data, targeted by chunker. Actual blobs can be smaller.
pub const BLOB_MINIMUM_TARGET_SIZE: usize = 256 * 1024; // 256 KiB

/// Desired size of blob data, targeted by chunker.
pub const BLOB_DESIRED_TARGET_SIZE: usize = 1 * 1024 * 1024; // 1 MiB
