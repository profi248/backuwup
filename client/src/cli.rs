use dialoguer::{Input, Select};
use dialoguer::theme::ColorfulTheme;
use owo_colors::OwoColorize;
use crate::setup;

pub async fn first_run_guide() {
    println!("{}", "Welcome!".yellow().bold());
    let items = vec!["Start fresh", "Restore"];
    match Select::with_theme(&ColorfulTheme::default())
        .items(&items)
        .default(0)
        .interact().expect("Failed to select option")
    {
        0 => {
            println!("{}", "Generating a secret and registering...".green());

            // todo improve error handling
            setup::setup(None).await.expect("Failed to setup!");

            println!("{}", "Success!".green());
        }
        1 => {
            println!("{}", "Restoring".green());
            let secret: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Please enter the secret that was generated during the initial setup")
                .interact().expect("Failed to enter secret");
            println!("Secret: {}", secret);
        }
        _ => {
            println!("Invalid option");
        }
    }
}

pub fn print_server_url(url: impl Into<String>) {
    println!("{}: http://{}", "UI is running at this address".bold().green(), url.into());
}
