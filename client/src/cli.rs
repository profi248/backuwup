use bip39::Mnemonic;
use dialoguer::{theme::ColorfulTheme, Input, Password, Select};
use owo_colors::OwoColorize;

use crate::{
    identity,
    key_manager::{MasterSecret, MASTER_SECRET_LENGTH},
    KEYS,
};

pub async fn first_run_guide() {
    // todo windows shell color escapes
    println!("{}", "Welcome!".yellow().bold());
    let items = vec!["Start fresh", "Restore"];
    match Select::with_theme(&ColorfulTheme::default())
        .items(&items)
        .default(0)
        .interact()
        .expect("Failed to select option")
    {
        0 => fresh_setup_guide().await,
        1 => restore_setup_guide().await,
        _ => unreachable!(),
    }
}

async fn restore_setup_guide() {
    println!("{}", "Restoring".green());

    // let the user enter the mnemonic, validate it, and try logging in
    let secret: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Please enter the mnemonic that was generated during the initial setup")
        .validate_with(|str: &String| -> Result<(), String> {
            let mnemonic = Mnemonic::parse(str).map_err(|e| e.to_string())?;

            if mnemonic.to_entropy().len() != MASTER_SECRET_LENGTH {
                return Err("Invalid length".to_string());
            }

            Ok(())
        })
        .interact()
        .expect("Failed to enter secret");

    let secret: MasterSecret = Mnemonic::parse(&secret)
        .unwrap()
        .to_entropy()
        .try_into()
        .unwrap();
    identity::existing_secret_setup(secret)
        .await
        .expect("Failed to setup!");

    println!();
    println!("{}", "Success!".green());
}

async fn fresh_setup_guide() {
    println!("{}", "Generating a secret and registering...".green());
    println!();

    // todo improve error handling
    identity::new_secret_setup().await.expect("Failed to setup!");

    let secret = KEYS.get().unwrap().get_master_secret();
    let mnemonic = Mnemonic::from_entropy(&secret).unwrap().to_string();

    println!("{}", "This is your mnemonic that is used as a password for backup restoration, please write it down and keep it safe!".bold().red());
    println!("{}", mnemonic.bold().bright_black().on_white());
    println!();

    // use password prompt as a blocker that doesn't echo the input
    Password::new()
        .with_prompt("Press enter to continue")
        .report(false)
        .allow_empty_password(true)
        .interact()
        .expect("Input failed");

    println!("{}", "Success!".green());
}

pub fn print_server_url(url: impl Into<String>) {
    println!("{}: http://{}", "UI is running at this address".bold().green(), url.into());
}
