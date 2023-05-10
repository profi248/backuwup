//! Shared constants for client and server.

/// Maximum size of a storage request.
pub const MAX_BACKUP_STORAGE_REQUEST_SIZE: u64 = 17_179_869_184; // 16 GiB

/// The expiry time for a storage request.
pub const BACKUP_REQUEST_EXPIRY: u64 = 5 * 60; // 5 minutes
