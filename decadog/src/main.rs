#![deny(clippy::all)]

use std::path::PathBuf;

use decadog_core::secret::Secret;
#[cfg(feature = "config_keyring")]
use keyring::Keyring;
use log::{debug, error};
use serde_derive::{Deserialize, Serialize};
use structopt::StructOpt;

mod args;
mod command;
mod error;
mod interact;

use args::{Args, Command};
use command::sprint;
pub use error::Error;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Settings {
    version: Option<u32>,
    owner: String,
    repo: String,
    github_url: String,
    github_token: Secret,
    zenhub_url: Option<String>,
    zenhub_token: Option<Secret>,
}

impl Settings {
    /// Load settings. If a `config_path` is given, it must exist.
    pub fn load(config_path: Option<PathBuf>) -> Result<Self, config::ConfigError> {
        debug!("Loading settings");

        let mut settings = config::Config::default();
        settings.set_default("github_url", "https://api.github.com/")?;
        settings.set_default("zenhub_url", "https://api.zenhub.io/")?;
        if let Some(config_path) = config_path {
            settings.merge(config::File::from(config_path).required(true))?;
        } else {
            settings.merge(config::File::with_name("decadog").required(false))?;
        }
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
        let settings = settings.try_into::<Self>()?;
        debug!("Loaded settings: {:?}", settings);
        Ok(settings)
    }
}

fn run(args: Args) -> Result<(), Error> {
    let settings = Settings::load(args.config)?;

    match args.command {
        Command::Sprint { ref command } => sprint::run(command, &settings),
    }
}

pub fn main() {
    env_logger::init();
    debug!("Initialised logger.");

    let args = Args::from_args();
    if let Err(error) = run(args) {
        error!("{}", error);
    }
}
