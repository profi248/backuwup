use crate::CONFIG;
use crate::key_manager::{KeyManager, MasterSecret};
use crate::net::requests;

pub async fn setup(secret: Option<MasterSecret>) -> anyhow::Result<KeyManager> {
    // if we have a master secret, use it to generate keypair
    let key_manager = if secret.is_some() {
        KeyManager::from_secret(secret.unwrap())
    } else {
        KeyManager::generate()
    }?;

    // begin transaction, so that if server registration fails,
    // we don't end up with an inconsistent state
    let mut transaction = CONFIG.get().unwrap().transaction().await?;

    // save master secret to disk
    transaction.save_master_secret(key_manager.get_master_secret()).await?;

    let pubkey = key_manager.get_pubkey();

    // perform registration to server with challenge-response
    let challenge_nonce = requests::register_begin(pubkey).await?;
    requests::register_complete(pubkey, key_manager.sign(&challenge_nonce)).await?;

    // registration complete, set initialized
    transaction.set_initialized().await?;
    transaction.commit().await?;

    // todo maybe put this in a OnceCell
    Ok(key_manager)
}
