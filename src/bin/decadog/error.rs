use std::io::Error as IoError;

use decadog::Error as DecadogError;
use scout::errors::Error as ScoutError;
use snafu::Snafu;

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub")]
pub enum Error {
    #[snafu(display("Decadog client error: {}", source))]
    Decadog { source: DecadogError },

    #[snafu(display("Scout error: {}", source))]
    Scout { source: ScoutError },

    #[snafu(display("Io error: {}", source))]
    Io { source: IoError },
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

pub type Result<T, E = Error> = std::result::Result<T, E>;
