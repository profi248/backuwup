use std::{future::Future, time::Duration};

use anyhow::{anyhow, bail};
use shared::{
    client_message::{
        BackupDone, BackupRequest, BackupRestoreRequest, BeginP2PConnectionRequest, ClientLoginAuth,
        ClientLoginRequest, ClientRegistrationAuth, ClientRegistrationRequest, ConfirmP2PConnectionRequest,
    },
    server_message::{BackupRestoreInfo, ClientLoginToken, ErrorType, ServerMessage},
    types::{BlobHash, ChallengeNonce, ClientId, SessionToken, TransportSessionNonce},
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

pub async fn login_complete(pubkey: ClientId, response: Signature) -> anyhow::Result<ClientLoginToken> {
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

pub async fn p2p_connection_begin(
    destination_client_id: ClientId,
    session_nonce: TransportSessionNonce,
) -> anyhow::Result<bool> {
    retry_with_login(|token| async move {
        let client = reqwest::Client::new();
        let response = client
            .post(url("p2p/connection/begin"))
            .json(&BeginP2PConnectionRequest {
                session_token: token,
                destination_client_id,
                session_nonce,
            })
            .send()
            .await?;

        match response.json().await? {
            ServerMessage::Ok => Ok(true),
            ServerMessage::Error(ErrorType::Unauthorized) => Err(ResponseError::Unauthorized),
            ServerMessage::Error(ErrorType::DestinationUnreachable) => Ok(false),
            ServerMessage::Error(e) => Err(ResponseError::Other(anyhow!("request failed: {e:?}"))),
            _ => Err(ResponseError::Other(anyhow!("unexpected response"))),
        }
    }).await
}

pub async fn p2p_connection_confirm(
    source_client_id: ClientId,
    destination_ip_address: String,
) -> anyhow::Result<()> {
    let ip = &destination_ip_address.clone();
    retry_with_login(|token| async move {
        let client = reqwest::Client::new();
        let response = client
            .post(url("p2p/connection/confirm"))
            .json(&ConfirmP2PConnectionRequest {
                session_token: token,
                source_client_id,
                destination_ip_address: ip.to_string(),
            })
            .send()
            .await?;

        match response.json().await? {
            ServerMessage::Ok => Ok(()),
            ServerMessage::Error(ErrorType::Unauthorized) => Err(ResponseError::Unauthorized),
            ServerMessage::Error(e) => Err(ResponseError::Other(anyhow!("request failed: {e:?}"))),
            _ => Err(ResponseError::Other(anyhow!("unexpected response"))),
        }
    }).await
}

pub async fn backup_storage_request(amount: u64) -> anyhow::Result<()> {
    retry_with_login(|token| async move {
        let client = reqwest::Client::new();
        let response = client
            .post(url("backups/request"))
            .json(&BackupRequest {
                session_token: token,
                storage_required: amount,
            })
            .send()
            .await?;

        match response.json().await? {
            ServerMessage::Ok => Ok(()),
            ServerMessage::Error(ErrorType::Unauthorized) => Err(ResponseError::Unauthorized),
            ServerMessage::Error(e) => Err(ResponseError::Other(anyhow!("request failed: {e:?}"))),
            _ => Err(ResponseError::Other(anyhow!("unexpected response"))),
        }
    }).await
}

pub async fn backup_done(snapshot_hash: BlobHash) -> anyhow::Result<()> {
    retry_with_login(|token| async move {
        let client = reqwest::Client::new();
        let response = client
            .post(url("backups/done"))
            .json(&BackupDone { session_token: token, snapshot_hash })
            .send()
            .await?;

        match response.json().await? {
            ServerMessage::Ok => Ok(()),
            ServerMessage::Error(ErrorType::Unauthorized) => Err(ResponseError::Unauthorized),
            ServerMessage::Error(e) => Err(ResponseError::Other(anyhow!("request failed: {e:?}"))),
            _ => Err(ResponseError::Other(anyhow!("unexpected response"))),
        }
    })
    .await?;

    Ok(())
}

pub async fn backup_restore_request() -> anyhow::Result<BackupRestoreInfo> {
    let info = retry_with_login(|token| async move {
        let client = reqwest::Client::new();
        let response = client
            .post(url("backups/restore"))
            .json(&BackupRestoreRequest { session_token: token })
            .send()
            .await?;

        match response.json().await? {
            ServerMessage::BackupRestoreInfo(info) => Ok(info),
            ServerMessage::Error(ErrorType::Unauthorized) => Err(ResponseError::Unauthorized),
            ServerMessage::Error(e) => Err(ResponseError::Other(anyhow!("request failed: {e:?}"))),
            _ => Err(ResponseError::Other(anyhow!("unexpected response"))),
        }
    })
    .await?;

    Ok(info)
}

async fn retry_with_login<T, F>(func: impl Fn(SessionToken) -> F) -> anyhow::Result<T>
where
    F: Future<Output = Result<T, ResponseError>>,
{
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

    bail!("unrecoverable auth error");
}

#[derive(thiserror::Error, Debug)]
#[error("error")]
enum ResponseError {
    Unauthorized,
    Network(#[from] reqwest::Error),
    Other(#[from] anyhow::Error),
}

fn url(s: impl Into<String>) -> String {
    // todo handle https
    // todo use config
    format!("http://{}/{}", crate::defaults::SERVER_URL, s.into())
}
