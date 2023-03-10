use crate::{
    key_manager::{KeyManager, MasterSecret},
    net::requests,
    CONFIG, KEYS,
};

pub async fn login() -> anyhow::Result<()> {
    let key_manager = KEYS.get().unwrap();
    let pubkey = key_manager.get_pubkey();

    // try to login to server with challenge-response
    let challenge_nonce = requests::login_begin(pubkey).await?;
    let token = requests::login_complete(pubkey, key_manager.sign(&challenge_nonce))
        .await?
        .token;

    // save the token to disk
    CONFIG.get().unwrap().save_auth_token(Some(token)).await?;

    Ok(())
}

pub async fn load_secret() -> anyhow::Result<()> {
    let secret = CONFIG.get().unwrap().load_master_secret().await?;
    KEYS.set(KeyManager::from_secret(secret)?)
        .expect("KeyManager already set");

    Ok(())
}

pub async fn existing_secret_setup(secret: MasterSecret) -> anyhow::Result<()> {
    let key_manager = KeyManager::from_secret(secret)?;

    let mut transaction = CONFIG.get().unwrap().transaction().await?;
    transaction
        .save_master_secret(key_manager.get_master_secret())
        .await?;

    let pubkey = key_manager.get_pubkey();

    // try to login to server with challenge-response, to verify that the secret is correct
    let challenge_nonce = requests::login_begin(pubkey).await?;
    requests::login_complete(pubkey, key_manager.sign(&challenge_nonce)).await?;

    // login successful, set initialized
    transaction.set_initialized().await?;
    transaction.commit().await?;

    // set key manager
    KEYS.set(key_manager).expect("KeyManager already set");

    Ok(())
}

pub async fn new_secret_setup() -> anyhow::Result<()> {
    let key_manager = KeyManager::generate()?;

    // begin transaction, so that if server registration fails,
    // we don't end up with an inconsistent state
    let mut transaction = CONFIG.get().unwrap().transaction().await?;

    // save master secret to disk
    transaction
        .save_master_secret(key_manager.get_master_secret())
        .await?;

    let pubkey = key_manager.get_pubkey();

    // perform registration to server with challenge-response
    let challenge_nonce = requests::register_begin(pubkey).await?;
    requests::register_complete(pubkey, key_manager.sign(&challenge_nonce)).await?;

    // registration complete, set initialized
    transaction.set_initialized().await?;
    transaction.commit().await?;

    // set key manager
    KEYS.set(key_manager).expect("KeyManager already set");

    Ok(())
}
