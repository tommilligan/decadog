use clap::App;
use config;
#[cfg(feature = "config_keyring")]
use keyring::Keyring;
use log::{debug, error};
use serde_derive::{Deserialize, Serialize};

mod error;
mod sprint;

pub use error::Error;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Settings {
    version: Option<u32>,
    owner: String,
    repo: String,
    github_token: String,
    zenhub_token: Option<String>,
}

impl Settings {
    pub fn load() -> Result<Settings, config::ConfigError> {
        debug!("Loading settings");

        let mut settings = config::Config::default();
        settings.merge(config::File::with_name("decadog").required(false))?;
        settings.merge(config::Environment::with_prefix("DECADOG"))?;

        #[cfg(feature = "config_keyring")]
        {
            const KEYRING_USERNAME: &str = "decadog";
            const KEYRING_GITHUB_TOKEN: &str = "decadog_github_token";
            const KEYRING_ZENHUB_TOKEN: &str = "decadog_zenhub_token";

            debug!("Loading credentials from keyring");
            let github_keyring = Keyring::new(KEYRING_GITHUB_TOKEN, KEYRING_USERNAME);
            if let Ok(token) = github_keyring.get_password() {
                settings.set("github_token", token)?;
            };
            let zenhub_keyring = Keyring::new(KEYRING_ZENHUB_TOKEN, KEYRING_USERNAME);
            if let Ok(token) = zenhub_keyring.get_password() {
                settings.set("zenhub_token", token)?;
            };
        }

        // Print out our settings (as a HashMap)
        let settings = settings.try_into::<Settings>()?;
        debug!("Loaded settings: {:?}", settings);
        Ok(settings)
    }
}

pub fn run() -> Result<(), Error> {
    let settings = Settings::load()?;

    let mut app = App::new("decadog")
        .about("Github toolkit. Octocat++.")
        .subcommand(sprint::subcommand());

    let matches = app.clone().get_matches();
    if let (subcommand_name, Some(subcommand_matches)) = matches.subcommand() {
        match subcommand_name {
            "sprint" => sprint::execute(subcommand_matches, &settings)?,
            _ => {
                error!("Subcommand '{}' not implemented.", subcommand_name);
            }
        }
    } else {
        app.print_help()?;
    }

    Ok(())
}

pub fn main() {
    env_logger::init();
    debug!("Initialised logger.");

    if let Err(error) = run() {
        error!("{}", error);
    }
}
