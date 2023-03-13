use anyhow::bail;
use shared::{
    client_message::{
        ClientLoginAuth, ClientLoginRequest, ClientRegistrationAuth, ClientRegistrationRequest,
    },
    server_message::{ClientLoginToken, ErrorType::Failure, ServerMessage},
    types::{ChallengeNonce, ClientId},
};

use crate::key_manager::Signature;

pub async fn register_begin(pubkey: ClientId) -> anyhow::Result<ChallengeNonce> {
    let client = reqwest::Client::new();

    let response = client
        .post(url("register/begin"))
        .json(&ClientRegistrationRequest { client_id: pubkey })
        .send()
        .await?;

    match response.json().await? {
        ServerMessage::ClientRegistrationChallenge(msg) => Ok(msg.server_challenge),
        ServerMessage::Error(Failure(e)) => bail!(e),
        _ => bail!("unexpected response"),
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
        ServerMessage::Error(Failure(e)) => bail!(e),
        _ => bail!("unexpected response"),
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
        ServerMessage::Error(Failure(e)) => bail!(e),
        _ => bail!("unexpected response"),
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
        ServerMessage::Error(Failure(e)) => bail!(e),
        _ => bail!("unexpected response"),
    }
}

fn url(s: impl Into<String>) -> String {
    // todo handle https
    // todo use config
    format!("http://{}/{}", crate::defaults::SERVER_URL, s.into())
}
