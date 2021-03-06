use reqwest::{Error as ReqwestError, StatusCode};
use snafu::Snafu;
use url::ParseError as UrlParseError;

use crate::github::GithubClientErrorBody;

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub")]
pub enum Error {
    #[snafu(display("Api error [{}]: {}", status, description))]
    Api {
        description: String,
        status: StatusCode,
    },

    #[snafu(display("Decadog config error: {}", description))]
    Config { description: String },

    #[snafu(display("Github error [{}]: {:?}", status, error))]
    Github {
        error: GithubClientErrorBody,
        status: StatusCode,
    },

    #[snafu(display("Reqwest error: {}", source))]
    Reqwest { source: ReqwestError },

    #[snafu(display("Url parse error: {}", source))]
    Url { source: UrlParseError },

    #[snafu(display("Unknown error: {}", description))]
    Unknown { description: String },
}

impl From<ReqwestError> for Error {
    fn from(source: ReqwestError) -> Self {
        Error::Reqwest { source }
    }
}

impl From<UrlParseError> for Error {
    fn from(source: UrlParseError) -> Self {
        Error::Url { source }
    }
}
