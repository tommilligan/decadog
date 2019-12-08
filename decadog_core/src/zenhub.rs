use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::Hasher;

use chrono::{DateTime, FixedOffset};
use log::debug;
use reqwest::header::HeaderMap;
use reqwest::{Client as ReqwestClient, Method, RequestBuilder, Url, UrlError};
use serde::de::DeserializeOwned;
use serde_derive::{Deserialize, Serialize};

use crate::error::Error;

pub struct Client {
    id: u64,
    reqwest_client: ReqwestClient,
    headers: HeaderMap,
    base_url: Url,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Zenhub client {}", self.id)
    }
}

/// Send a HTTP request to an API, and return the resulting struct.
trait SendApiExt {
    fn send_api<T>(self) -> Result<T, Error>
    where
        Self: Sized,
        T: DeserializeOwned;

    fn send_api_no_response(self) -> Result<(), Error>
    where
        Self: Sized;
}

/// Send a HTTP request to an API, and return the resulting struct.
impl SendApiExt for RequestBuilder {
    fn send_api<T>(self) -> Result<T, Error>
    where
        Self: Sized,
        T: DeserializeOwned,
    {
        let mut response = self.send()?;
        let status = response.status();
        if status.is_success() {
            Ok(response.json()?)
        } else if status.is_client_error() {
            Err(Error::Api {
                description: response.text()?,
                status,
            })
        } else {
            Err(Error::Api {
                description: "Unexpected response status code.".to_owned(),
                status,
            })
        }
    }

    fn send_api_no_response(self) -> Result<(), Error>
    where
        Self: Sized,
    {
        let mut response = self.send()?;
        let status = response.status();
        if status.is_success() {
            Ok(())
        } else if status.is_client_error() {
            Err(Error::Api {
                description: response.text()?,
                status,
            })
        } else {
            Err(Error::Api {
                description: "Unexpected response status code.".to_owned(),
                status,
            })
        }
    }
}

impl Client {
    /// Create a new client that can make requests to the Zenhub API using token auth.
    pub fn new(url: &str, token: &str) -> Result<Client, Error> {
        // Create reqwest client to interact with APIs
        // TODO: should we pass in an external client here?
        let reqwest_client = reqwest::Client::new();

        let mut headers = HeaderMap::new();
        headers.insert(
            "x-authentication-token",
            token.parse().map_err(|_| Error::Config {
                description: "Invalid Zenhub token for Authentication header.".to_owned(),
            })?,
        );

        let base_url = Url::parse(url).map_err(|_| Error::Config {
            description: format!("Invalid Zenhub base url {}", url),
        })?;

        let mut hasher = DefaultHasher::new();
        hasher.write(url.as_bytes());
        hasher.write(token.as_bytes());
        let id = hasher.finish();

        Ok(Client {
            id,
            reqwest_client,
            headers,
            base_url,
        })
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    /// Returns a `request::RequestBuilder` authorized to the Zenhub API.
    pub fn request(&self, method: Method, url: Url) -> Result<RequestBuilder, UrlError> {
        debug!("{} {}", method, url.as_str());
        Ok(self
            .reqwest_client
            .request(method, url)
            .headers(self.headers.clone()))
    }

    /// Get Zenhub board for a repository.
    pub fn get_board(&self, repository_id: u64) -> Result<Board, Error> {
        self.request(
            Method::GET,
            self.base_url
                .join("/p1/repositories")?
                .join(&repository_id.to_string())?
                .join("board")?,
        )?
        .send_api()
    }

    /// Get Zenhub StartDate for a milestone.
    pub fn get_start_date(
        &self,
        repository_id: u64,
        milestone_number: u32,
    ) -> Result<StartDate, Error> {
        self.request(
            Method::GET,
            self.base_url.join(&format!(
                "/p1/repositories/{}/milestones/{}/start_date",
                repository_id, milestone_number
            ))?,
        )?
        .send_api()
    }

    /// Get Zenhub issue metadata.
    pub fn get_issue(&self, repository_id: u64, issue_number: u32) -> Result<Issue, Error> {
        self.request(
            Method::GET,
            self.base_url.join(&format!(
                "/p1/repositories/{}/issues/{}",
                repository_id, issue_number
            ))?,
        )?
        .send_api()
    }

    /// Set Zenhub issue estimate.
    pub fn set_estimate(
        &self,
        repository_id: u64,
        issue_number: u32,
        estimate: u32,
    ) -> Result<(), Error> {
        self.request(
            Method::PUT,
            self.base_url.join(&format!(
                "/p1/repositories/{}/issues/{}/estimate",
                repository_id, issue_number
            ))?,
        )?
        .json(&SetEstimate::from(estimate))
        .send_api_no_response()
    }

    /// Move issue to a Zenhub pipeline.
    pub fn move_issue(
        &self,
        repository_id: u64,
        issue_number: u32,
        position: &PipelinePosition,
    ) -> Result<(), Error> {
        self.request(
            Method::POST,
            self.base_url.join(&format!(
                "/p1/repositories/{}/issues/{}/moves",
                repository_id, issue_number
            ))?,
        )?
        .json(position)
        .send_api_no_response()
    }
}

/// Zenhub issue data.
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct Issue {
    pub estimate: Option<Estimate>,
    pub is_epic: bool,
}

/// A Zenhub estimate.
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct Estimate {
    pub value: u32,
}

/// Body to set a Zenhub estimate.
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct SetEstimate {
    pub estimate: u32,
}

impl From<u32> for SetEstimate {
    fn from(estimate: u32) -> Self {
        SetEstimate { estimate }
    }
}

/// A Zenhub reference to an issue.
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct PipelineIssue {
    pub issue_number: u32,
    pub estimate: Option<Estimate>,
    pub is_epic: bool,
    pub position: u32,
}

/// A Zenhub pipeline.
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct Pipeline {
    pub id: String,
    pub name: String,
    pub issues: Vec<PipelineIssue>,
}

/// A position of an issue in a Zenhub pipeline.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct PipelinePosition {
    pub pipeline_id: String,
    pub position: String,
}

impl Default for PipelinePosition {
    fn default() -> Self {
        Self {
            pipeline_id: Default::default(),
            position: "top".to_owned(),
        }
    }
}

/// A Zenhub board.
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct Board {
    pub pipelines: Vec<Pipeline>,
}

/// A Zenhub milestone StartDate.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct StartDate {
    pub start_date: DateTime<FixedOffset>,
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn invalid_token() {
        assert!(Client::new("https://api.myzenhub.com/", "zenhub_token").is_ok());
        match Client::new("https://api.myzenhub.com/", "invalid header char -> \n").unwrap_err() {
            Error::Config { description } => assert_eq!(
                description,
                "Invalid Zenhub token for Authentication header."
            ),
            _ => panic!("Unexpected error"),
        }
    }
}