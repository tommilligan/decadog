use std::path::PathBuf;
use structopt::StructOpt;

use crate::command::sprint::Command as SprintCommand;

/// Github and Zenhub toolkit. Octocat++.
#[derive(Debug, StructOpt)]
#[structopt(
    global_settings = &[
        structopt::clap::AppSettings::ColoredHelp,
        structopt::clap::AppSettings::InferSubcommands,
    ]
)]
pub struct Args {
    #[structopt(long = "config", parse(from_os_str))]
    /// Defaults to ./decadog.yml
    pub config: Option<PathBuf>,

    /// Subcommand selected.
    #[structopt(subcommand)]
    pub command: Command,
}

#[derive(Debug, StructOpt)]
pub enum Command {
    #[structopt(name = "sprint")]
    /// Manage sprints.
    Sprint {
        #[structopt(subcommand)]
        command: SprintCommand,
    },
}
