use std::io::Error as IoError;

use clap::Error as ClapError;
use config::ConfigError;
use decadog::Error as DecadogError;
use scout::errors::Error as ScoutError;
use snafu::Snafu;

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub")]
pub enum Error {
    #[snafu(display("Clap error: {}", source))]
    Clap { source: ClapError },

    #[snafu(display("Config error: {}", source))]
    Config { source: ConfigError },

    #[snafu(display("Decadog client error: {}", source))]
    Decadog { source: DecadogError },

    #[snafu(display("Scout error: {}", source))]
    Scout { source: ScoutError },

    #[snafu(display("Io error: {}", source))]
    Io { source: IoError },

    #[snafu(display("Invalid settings: {}", description))]
    Settings { description: String },
}

impl From<ClapError> for Error {
    fn from(source: ClapError) -> Self {
        Error::Clap { source }
    }
}

impl From<ConfigError> for Error {
    fn from(source: ConfigError) -> Self {
        Error::Config { source }
    }
}

impl From<DecadogError> for Error {
    fn from(source: DecadogError) -> Self {
        Error::Decadog { source }
    }
}

impl From<ScoutError> for Error {
    fn from(source: ScoutError) -> Self {
        Error::Scout { source }
    }
}

impl From<IoError> for Error {
    fn from(source: IoError) -> Self {
        Error::Io { source }
    }
}
