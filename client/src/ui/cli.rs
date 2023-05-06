//! Implements the CLI for initial setup and restore.

use bip39::Mnemonic;
use dialoguer::{theme::ColorfulTheme, Input, Password, Select};
use owo_colors::OwoColorize;

use crate::{
    identity,
    key_manager::RootSecret,
    KEYS,
};

/// Handle the first start guide in a CLI, which allows the user to either start fresh or restore from a mnemonic.
pub async fn first_run_guide() {
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

/// Display the restore guide in a CLI, which allows the user to enter a mnemonic and restore from it.
async fn restore_setup_guide() {
    println!("{}", "Restoring".green());

    // let the user enter the mnemonic, validate it, and try logging in
    let secret: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Please enter the mnemonic that was generated during the initial setup")
        .validate_with(|str: &String| -> Result<(), String> {
            let mnemonic = Mnemonic::parse(str).map_err(|e| e.to_string())?;

            if RootSecret::try_from(mnemonic.to_entropy()).is_err() {
                return Err("Invalid length".to_string());
            }

            Ok(())
        })
        .interact()
        .expect("Failed to enter secret");

    let secret: RootSecret = Mnemonic::parse(&secret).unwrap().to_entropy().try_into().unwrap();
    identity::existing_secret_setup(secret)
        .await
        .expect("Failed to setup!");

    println!();
    println!("{}", "Success!".green());
}

/// Display the fresh setup guide in a CLI, showing the user a newly generated mnemonic and
/// prompting them to write it down.
async fn fresh_setup_guide() {
    println!("{}", "Generating a secret and registering...".green());
    println!();

    identity::new_secret_setup().await.expect("Failed to setup!");

    let secret = KEYS.get().unwrap().get_root_secret();
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
