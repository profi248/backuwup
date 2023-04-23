use getrandom::getrandom;

use crate::{
    key_manager::{KeyManager, RootSecret},
    net_server::requests,
    CONFIG, KEYS,
};

/// Perform a login to the server, using the stored root secret, and save the token to disk.
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

/// Initialize the key manager.
pub async fn load_secret() -> anyhow::Result<()> {
    let secret = CONFIG.get().unwrap().load_root_secret().await?;
    KEYS.set(KeyManager::from_secret(secret)?)
        .expect("KeyManager already set");

    Ok(())
}

/// Generate a random 4-byte backup storage obfuscation key.
pub fn generate_obfuscation_key() -> anyhow::Result<u32> {
    let mut key_bytes = [0u8; 4];
    getrandom(&mut key_bytes)?;

    Ok(u32::from_le_bytes(key_bytes))
}

/// Performs an initial setup process, given an existing seed to restore from.
pub async fn existing_secret_setup(secret: RootSecret) -> anyhow::Result<()> {
    let key_manager = KeyManager::from_secret(secret)?;

    let mut transaction = CONFIG.get().unwrap().transaction().await?;
    transaction
        .save_root_secret(key_manager.get_root_secret())
        .await?;

    let pubkey = key_manager.get_pubkey();

    // try to login to server with challenge-response, to verify that the secret is correct
    let challenge_nonce = requests::login_begin(pubkey).await?;
    requests::login_complete(pubkey, key_manager.sign(&challenge_nonce)).await?;

    // generate a new obfuscation key
    transaction.save_obfuscation_key(generate_obfuscation_key()?).await?;

    // login successful, set initialized
    transaction.set_initialized().await?;
    transaction.commit().await?;

    // set key manager
    KEYS.set(key_manager).expect("KeyManager already set");

    Ok(())
}

/// Performs an initial setup process, generating a new root secret.
pub async fn new_secret_setup() -> anyhow::Result<()> {
    let key_manager = KeyManager::generate()?;

    // begin transaction, so that if server registration fails,
    // we don't end up with an inconsistent state
    let mut transaction = CONFIG.get().unwrap().transaction().await?;

    // save root secret to disk
    transaction
        .save_root_secret(key_manager.get_root_secret())
        .await?;

    let pubkey = key_manager.get_pubkey();

    // perform registration to server with challenge-response
    let challenge_nonce = requests::register_begin(pubkey).await?;
    requests::register_complete(pubkey, key_manager.sign(&challenge_nonce)).await?;

    // generate a new obfuscation key
    transaction.save_obfuscation_key(generate_obfuscation_key()?).await?;

    // registration complete, set initialized
    transaction.set_initialized().await?;
    transaction.commit().await?;

    // set key manager
    KEYS.set(key_manager).expect("KeyManager already set");

    Ok(())
}
