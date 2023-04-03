use std::time::Duration;

use anyhow::bail;
use shared::{
    client_message::{
        BackupRequest, BeginTransportRequest, ClientLoginAuth, ClientLoginRequest,
        ClientRegistrationAuth, ClientRegistrationRequest, ConfirmTransportRequest,
    },
    server_message::{ClientLoginToken, ErrorType, ServerMessage},
    types::{ChallengeNonce, ClientId, TransportSessionNonce},
};

use crate::{identity, key_manager::Signature, CONFIG};

pub async fn register_begin(pubkey: ClientId) -> anyhow::Result<ChallengeNonce> {
    let client = reqwest::Client::new();

    let response = client
        .post(url("register/begin"))
        .json(&ClientRegistrationRequest { client_id: pubkey })
        .send()
        .await?;

    match response.json().await? {
        ServerMessage::ClientRegistrationChallenge(msg) => Ok(msg.server_challenge),
        ServerMessage::Error(e) => bail!(format!("Request failed: {e:?}")),
        _ => bail!("Unexpected response"),
    }
}

pub async fn register_complete(pubkey: ClientId, response: Signature) -> anyhow::Result<()> {
    let client = reqwest::Client::new();

    let response = client
        .post(url("register/complete"))
        .json(&ClientRegistrationAuth {
            client_id: pubkey,
            challenge_response: Vec::from(response),
        })
        .send()
        .await?;

    match response.json().await? {
        ServerMessage::Ok => Ok(()),
        ServerMessage::Error(e) => bail!(format!("Request failed: {e:?}")),
        _ => bail!("Unexpected response"),
    }
}

pub async fn login_begin(pubkey: ClientId) -> anyhow::Result<ChallengeNonce> {
    let client = reqwest::Client::new();

    let response = client
        .post(url("login/begin"))
        .json(&ClientLoginRequest { client_id: pubkey })
        .send()
        .await?;

    match response.json().await? {
        ServerMessage::ClientLoginChallenge(msg) => Ok(msg.server_challenge),
        ServerMessage::Error(e) => bail!(format!("Request failed: {e:?}")),
        _ => bail!("Unexpected response"),
    }
}

pub async fn login_complete(
    pubkey: ClientId,
    response: Signature,
) -> anyhow::Result<ClientLoginToken> {
    let client = reqwest::Client::new();

    let response = client
        .post(url("login/complete"))
        .json(&ClientLoginAuth {
            client_id: pubkey,
            challenge_response: Vec::from(response),
        })
        .send()
        .await?;

    match response.json().await? {
        ServerMessage::ClientLoginToken(token) => Ok(token),
        ServerMessage::Error(e) => bail!(format!("Request failed: {e:?}")),
        _ => bail!("Unexpected response"),
    }
}

pub async fn backup_transport_begin(
    destination_client_id: ClientId,
    session_nonce: TransportSessionNonce,
) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let config = CONFIG.get().unwrap();

    // login reattempt was supposed to be in a separate function, but I was not able to convince
    // the compiler to take an async closure as a parameter and gave up for now
    for _ in 0..2 {
        let token = config.load_auth_token().await?;
        if token.is_some() {
            let response = client
                .post(url("backups/transport/begin"))
                .json(&BeginTransportRequest {
                    session_token: token.unwrap(),
                    destination_client_id,
                    session_nonce,
                })
                .send()
                .await?;

            match response.json().await? {
                ServerMessage::Ok => return Ok(()),
                ServerMessage::Error(ErrorType::Unauthorized) => {
                    config.save_auth_token(None).await?;
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
                ServerMessage::Error(e) => bail!(format!("Request failed: {e:?}")),
                _ => bail!("Unexpected response"),
            };
        } else {
            identity::login().await?;
        }
    }

    bail!("Unrecoverable auth error");
}

pub async fn backup_transport_confirm(
    source_client_id: ClientId,
    destination_ip_address: String,
) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let config = CONFIG.get().unwrap();

    // login reattempt was supposed to be in a separate function, but I was not able to convince
    // the compiler to take an async closure as a parameter and gave up for now
    for _ in 0..2 {
        let token = config.load_auth_token().await?;
        if token.is_some() {
            let response = client
                .post(url("backups/transport/confirm"))
                .json(&ConfirmTransportRequest {
                    session_token: token.unwrap(),
                    source_client_id,
                    destination_ip_address: destination_ip_address.clone(),
                })
                .send()
                .await?;

            match response.json().await? {
                ServerMessage::Ok => return Ok(()),
                ServerMessage::Error(ErrorType::Unauthorized) => {
                    config.save_auth_token(None).await?;
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
                ServerMessage::Error(e) => bail!(format!("Request failed: {e:?}")),
                _ => bail!("Unexpected response"),
            };
        } else {
            identity::login().await?;
        }
    }

    bail!("Unrecoverable auth error");
}

pub async fn backup_storage_request(amount: u64) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let config = CONFIG.get().unwrap();

    // todo fix these
    // login reattempt was supposed to be in a separate function, but I was not able to convince
    // the compiler to take an async closure as a parameter and gave up for now
    for _ in 0..2 {
        let token = config.load_auth_token().await?;
        if token.is_some() {
            let response = client
                .post(url("backups/request"))
                .json(&BackupRequest {
                    session_token: token.unwrap(),
                    storage_required: amount,
                })
                .send()
                .await?;

            match response.json().await? {
                ServerMessage::Ok => return Ok(()),
                ServerMessage::Error(ErrorType::Unauthorized) => {
                    config.save_auth_token(None).await?;
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
                ServerMessage::Error(e) => bail!(format!("Request failed: {e:?}")),
                _ => bail!("Unexpected response"),
            };
        } else {
            identity::login().await?;
        }
    }

    bail!("Unrecoverable auth error");
}

/*
async fn retry_with_login<T, F>(func: impl FnOnce(SessionToken) -> F) -> anyhow::Result<T> where F: Future<Output = Result<T, ResponseError>> {
    // retry a few times with a limit
    for _ in 0..2 {
        let config = CONFIG.get().unwrap();
        let token = config.load_auth_token().await?;
        if token.is_some() {
            match func(token.unwrap()).await {
                Ok(val) => return Ok(val),
                Err(ResponseError::Unauthorized) => config.save_auth_token(None).await?,
                Err(ResponseError::Other(e)) => return Err(e),
                Err(ResponseError::Network(e)) => return Err(e.into()),
            }

            tokio::time::sleep(Duration::from_secs(2)).await;
        } else {
            identity::login().await?;
        }
    }

    bail!("Unrecoverable auth error");
}

#[derive(thiserror::Error, Debug)]
#[error("error")]
enum ResponseError {
    Unauthorized,
    Network(#[from] reqwest::Error),
    Other(#[from] anyhow::Error)
}
*/

fn url(s: impl Into<String>) -> String {
    // todo handle https
    // todo use config
    format!("http://{}/{}", crate::defaults::SERVER_URL, s.into())
}
