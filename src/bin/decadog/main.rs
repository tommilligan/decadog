use clap::App;
use log::error;

mod error;
mod sprint;

pub use error::{Error, Result};

pub fn main() -> Result<(), Error> {
    env_logger::init();

    let mut app = App::new("decadog")
        .about("Github toolkit. Octocat++.")
        .subcommand(sprint::subcommand());
    let matches = app.clone().get_matches();

    if let (subcommand_name, Some(subcommand_matches)) = matches.subcommand() {
        match subcommand_name {
            "sprint" => sprint::execute(subcommand_matches)?,
            _ => {
                error!("Invalid subcommand.");
                return Ok(());
            }
        }
    } else {
        app.print_help().expect("Could not print help.");
    }

    Ok(())
}
