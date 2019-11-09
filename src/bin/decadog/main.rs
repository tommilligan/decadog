use clap::App;
use config;
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
}

impl Settings {
    pub fn load() -> Result<Settings, config::ConfigError> {
        let mut settings = config::Config::default();
        settings.merge(config::File::with_name("decadog").required(false))?;
        settings.merge(config::Environment::with_prefix("DECADOG"))?;

        // Print out our settings (as a HashMap)
        let settings = settings.try_into::<Settings>()?;
        debug!("Loaded settings: {:?}", settings);
        Ok(settings)
    }
}

pub fn main() -> Result<(), Error> {
    env_logger::init();
    debug!("Initialised logger.");

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
