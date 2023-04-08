pub const SERVER_URL: &str = "localhost:8080";
pub const UI_BIND_ADDR: &str = "127.0.0.1:3000";
pub const APP_FOLDER_NAME: &str = "p2p-backup";
pub const BACKUP_BUFFER_FOLDER_NAME: &str = "local_packfiles";
pub const CONFIG_DB_FILE: &str = "config.db";

pub const PACKFILE_FOLDER: &str = "pack";
pub const INDEX_FOLDER: &str = "index";

pub const RECEIVED_PACKFILES_FOLDER: &str = "received_packfiles";

pub const PEER_STORAGE_USAGE_SPREAD: u64 = 32 * 1024 * 1024; // 32 MiB

/// Maximum size of packfiles that are allowed to be temporarily stored on disk,
/// while waiting for transferring them to a peer.
//pub const MAX_PACKFILE_LOCAL_BUFFER_SIZE: usize = 4 * 1024 * 1024 * 1024; // 4 GiB
pub const MAX_PACKFILE_LOCAL_BUFFER_SIZE: u64 = 100 * 1024 * 1024;

/// Maximum amount of seconds to wait until considering packfile transfer as failed.
pub const PACKFILE_SEND_TIMEOUT: u64 = 20;

/// Maximum amount of seconds to wait until considering packfile ack as failed.
pub const PACKFILE_ACK_TIMEOUT: u64 = 5;

/// The amount of free space under the packfile maximum local buffer size to trigger a backup resume.
pub const PACKFILE_LOCAL_BUFFER_RESUME_THRESHOLD: u64 = 50 * 1024 * 1024;

/// Maximum size of blob data that's allowed in a packfile.
pub const BLOB_MAX_UNCOMPRESSED_SIZE: usize = 3 * 1024 * 1024; // 3 MiB

/// Minimum size of blob data, targeted by chunker. Actual blobs can be smaller.
pub const BLOB_MINIMUM_TARGET_SIZE: usize = 256 * 1024; // 256 KiB

/// Desired size of blob data, targeted by chunker.
pub const BLOB_DESIRED_TARGET_SIZE: usize = 1 * 1024 * 1024; // 1 MiB

pub const SERVER_ROOT_TLS_CERT_PEM: &str = "\
-----BEGIN CERTIFICATE-----
MIIFsTCCA5mgAwIBAgIUFdA9iEgohQg+/FzyLh+4zqGGv1AwDQYJKoZIhvcNAQEL
BQAwczELMAkGA1UEBhMCQ1oxFzAVBgNVBAgMDkN6ZWNoIFJlcHVibGljMQwwCgYD
VQQHDANOL0ExFTATBgNVBAoMDERhdmlkIEtvc3RhbDEmMCQGA1UEAwwdUDJQIGJh
Y2t1cCBzZXJ2ZXIgY2VydGlmaWNhdGUwHhcNMjMwMjI2MjEzMDM0WhcNMjUwMjI1
MjEzMDM0WjBzMQswCQYDVQQGEwJDWjEXMBUGA1UECAwOQ3plY2ggUmVwdWJsaWMx
DDAKBgNVBAcMA04vQTEVMBMGA1UECgwMRGF2aWQgS29zdGFsMSYwJAYDVQQDDB1Q
MlAgYmFja3VwIHNlcnZlciBjZXJ0aWZpY2F0ZTCCAiIwDQYJKoZIhvcNAQEBBQAD
ggIPADCCAgoCggIBAKSLIfSeMRhewog5VRIgra0FLspPIcqAPKWingBkBrFjuitg
qof2IatScyqVEUwORUuR2Ik3edVB9HA9PWHqWjhAyHwVQ9D4UhcZfZk2PF1XgByO
z7oLaQX1Im3nbz2j3fHx8WSoKtir34J0xvtF3RjoQVlhjvtJ4Sb4bdl94BHhVAN9
6h7O6uMz8LU5pUqV1e+46RqEu+0C0fpc8YqM4cHvY5YpilVpzTZGs53MQhxfwKky
Dz/GjeA+7RtsFIUjVTfoS9kJpZbNunDGvM5W8VKcwYBtwL6eDzeaDUSleGl44rix
ZT9URZ7H77rsc3ZZiXHwAwT3/n9fueDUdSQfTSAD9FzTGxjxIMKmLbySPVln/cER
8jor48CGXsYp134aXeUNYJC0ecZa2CB+6Sc1/Eq/l8z+Cy8EBgHt0/c8LdSGVHbH
06yx/npIDaE/bSceCtzLRx8sDqwysPACaH2RODgVBCLaPUBdDwm/fBb+INOBT78S
iIs9MaIX4fuk17hBcrgRgeiwLYEbh9eJfY1ox9KfYaurpAZTdJfd88oRB/LTuSUu
XAJPuWR1njPk5Fm7Hkz5BNCESjxMrPGmm8WKToSDp6ghbVmBjmxDYcRUKzD4WRHT
aoVXNSccvL0aGZ/wyvSgZk04MHTpUY3Eb28Lotp0vyLZJ4yDsgrFWFnbjS75AgMB
AAGjPTA7MBoGA1UdEQQTMBGHBH8AAAGCCWxvY2FsaG9zdDAdBgNVHQ4EFgQUNrpf
NxviOmhGxqcRtB176HIOB8AwDQYJKoZIhvcNAQELBQADggIBAHejTgGA9+lkmIl+
bD1LFF3+ujule7/E7zwL6qy0vBUDzupDjkyKJiHHr/2CWVtMKJkzXPMpThUtjs50
ZepFu4H74m+uvsfTVnWntfHvhA/Sy7GmwEddIt72a7yPFVXyUWMNHeUS6Cr73eV9
moEWoMtQp3CgV+f3OsJOs5BnKpePiiDFI0ggfcGe2BT1lI8gVtidr5LzFrPCMs6O
KifqjHPP/zLNd+9HYBEdJ2uruUsVxZqdvk0WWswiz32tv0s6mLsrIIvafrL5sPmD
KKOfnk3iCZt/JKoWPyaC3t7pHlfQ33pQPnUgLetDBMt4XIqkC+/1rx0E/bfO09Yh
0zwjwxTAP1XgqPEhzkxHiif4jGmrj7sN8+Wk6Vv3/AuqQQ0+/aRL2t+4uLCUd0wY
sKlw27YC+m++WXGhS+GOHTac2bEHqkYsBvYTHQHQw994ipQUljvmAA0Xdpq3JRws
nOVBvQTTW51oYr5lM7DHtGdpvoiuTlDg2IRQyoHnKJhQ1T+FX3rIuqioS7S5FLmk
USIraxqDYwnFH0UBbSqlRz3kq9s7pufv9807ok9RTNn8IVg0vthObA+MoY0DxAX3
N+WEmiKRswrnc1jbaWPcnZbB5Uu4se1eqKpcc02ACsuviibh2K2RGLvnYfwwtyDB
q3TG2AtnNIEey5pcujyPS8MewZ1b
-----END CERTIFICATE-----";
