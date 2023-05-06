# Client user guide

This documentation will explain how to install and use the client application.

## Installation
The package contains precompiled executables for Linux (built on Fedora Linux 37, it might not run on other distributions) and Windows (tested on Windows 10). When on supported systems, simply run the executable in a terminal, where a first start guide should be displayed. After setting up the application for the first time, the web user interface will be available.

### Building from source
The application can also be built from source. To build it, you will need the [Rust toolchain](https://www.rust-lang.org/tools/install). However, it's easy to install by following the link and running the provided command.

After installing the Rust toolchain, the application (including all Rust libraries) automatically be built and started by running the following command. Make sure that you are in the `client` folder.

```bash
cargo run --release
```

The client application relies on SQLite. If you don't have the library installed already, the build might fail, and it will be necessary to get it manually. On Linux it should be sufficient to install packages from the repository, usually named `libsqlite3-dev` and `libsqlite3` for Debian-based distributions or `sqlite-devel` and `sqlite` for Red Hat-based distributions. On Windows, try following [this guide](https://github.com/rusqlite/rusqlite#notes-on-building-rusqlite-and-libsqlite3-sys).

## Usage
The client application needs to connect to a server. The default server address is `127.0.0.1:9999`, but it can be overridden with the environment variable `SERVER_ADDR`.

Backuwup client expects to run in a terminal environment and with ANSI color escapes supported (works on common Linux and modern Windows versions). When the application is started for the first time, a command line first start guide will show up. The user can choose to either set up the application from scratch (when creating a new backup) or from an existing mnemonic (for restoring an existing backup). When starting from scratch, it's necessary to save the displayed mnemonic to be able to perform a restore later if the application data is destroyed.

After the first setup guide is completed, a web user interface is started. To access it, simply open the displayed link in a browser. The default address of the web user interface is `http://127.0.0.1:3000` (and is accessible only on the local computer). This can be overridden by the environment variable `UI_BIND_ADDR`. For example, setting it to `0.0.0.0:3001` will change the port to `3001` and will allow access not only from the local computer.

The web user interface offers easy access to main functions with buttons at the top and displays basic progress graphically. Below the panel at the top, there's a window that shows logs. The terminal window where the application is running will show even more detailed logs.

### Backups and restores
Before starting a backup or a restore, a path needs to be set. That can be done in the right section of the user interface. 

#### Backups
After a backup is started, backuwup will begin scanning the path and packaging files for transport.

##### Storage requests
After starting a backup, the client will attempt to negotiate storage space with other peers (which is done via the server). If no peers are available for a backup, the backup will be paused, and the client will wait. Packaging files and transporting them will be done simultaneously, up to a certain specified maximum size of local files. 

The client will send a storage request to the server right after a backup starts. If there's no other request received by the server, it will be put in a queue and wait for storage requests from other clients. After the server receives other requests, they will be matched. If one request is for a greater amount of space than the other, it will only match up to the smaller size, and the unfulfilled part will get enqueued again and wait for other requests to come in.

> **Note for testing**
> 
> For creating backups, it's easiest to have two clients which have data of about the same size in their backup path. After starting a backup on one client, one can see that files start getting packaged and then the backup pauses. Starting a backup on the second client will then send another storage request to the server, they will get matched, and both clients will be able to complete the backup and successfully transport data to the other peer.

After a backup is completed, the snapshot ID of the completed backup will be sent to the server.

#### Peer-to-peer communication
The clients can only connect to each other if they are on the same local network and a firewall is not blocking a direct connection. The backuwup client will attempt to get the local IP address and a random port, which will be relayed through a server. 

#### Restores
Triggering a backup restore will first request and retrieve all files from all contacted peers. After all packaged data is retrieved, it will be unpacked into the backup path.

Currently, when restoring a backup, backuwup will attempt to contact **all** peers with any negotiated storage, no matter how many files were saved to that peer. For that reason, a client needs to be able to connect to all previously used peers to successfully restore a backup.

## Notes
Application data is stored in the following paths:

|Type|Linux|Windows|
|----|-----|--------|
|Configuration files|`$XDG_CONFIG_HOME/backuwup` or `$HOME/.config/backuwup`|`%LocalAppData%/backuwup`|
|Data from other clients and temporary backup/restore files|`$XDG_DATA_HOME/backuwup` or `$HOME/.local/share/backuwup`|`%LocalAppData%/backuwup`|
