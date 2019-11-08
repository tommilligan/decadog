use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::Hasher;

use github_rs::client::{Executor, Github};
use github_rs::StatusCode;
use reqwest::header::{HeaderMap, AUTHORIZATION};
use reqwest::{Client as ReqwestClient, RequestBuilder};
use serde::de::DeserializeOwned;
use serde_derive::{Deserialize, Serialize};

mod core;

pub use crate::core::{AssignedTo, Issue, Milestone, OrganisationMember};

/// Updates an Issue milestone.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct IssuePatchMilestone {
    pub milestone: u32,
}

/// Updates an Issue assignees.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct IssuePatchAssignees {
    pub assignees: Vec<String>,
}

/// Decadog client, used to abstract complex tasks over the Github API.
pub struct Client {
    id: u64,
    github_client: Github,
    reqwest_client: ReqwestClient,
    reqwest_headers: HeaderMap,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Decadog client {}", self.id)
    }
}

impl PartialEq for Client {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

trait TryExecute: Executor {
    fn try_execute<'de, T>(self) -> Result<T, String>
    where
        Self: Sized,
        T: DeserializeOwned,
    {
        #[derive(Deserialize)]
        struct GithubError {
            message: String,
        }

        match self.execute() {
            Ok((_, StatusCode::OK, Some(response))) => serde_json::from_value::<T>(response)
                .map_err(|err| format!("Failed to parse value response: {}", err))
                .and_then(|value| Ok(value)),
            Ok((_, _, Some(response))) => serde_json::from_value::<GithubError>(response)
                .map_err(|err| format!("Failed to parse error response: {}", err))
                .and_then(|error| Err(error.message.into())),
            Ok((_, _, None)) => Err("Received error response from github with no message".into()),
            Err(err) => Err(format!("Failed to execute request: {}", err)),
        }
    }
}

impl<'a> TryExecute for ::github_rs::repos::get::IssuesNumber<'a> {}
impl<'a> TryExecute for ::github_rs::orgs::get::OrgsOrgMembers<'a> {}

trait TrySend {
    fn try_send<T>(self) -> Result<T, Error>
    where
        Self: Sized,
        T: DeserializeOwned;
}

impl TrySend for RequestBuilder {
    fn try_send<T>(self) -> Result<T, Error>
    where
        Self: Sized,
        T: DeserializeOwned,
    {
        Ok(self.send()?.json()?)
    }
}

impl Client {
    /// Create a new client that can make requests to the Github API using token auth.
    pub fn new(token: &str) -> Result<Client, Error> {
        // Nice github API
        let github_client = Github::new(token)?;

        // Raw REST endpoints
        let reqwest_client = reqwest::Client::new();
        let mut reqwest_headers = HeaderMap::new();
        reqwest_headers.insert(
            AUTHORIZATION,
            format!("token {}", token)
                .parse()
                .map_err(|_| Error::BadRequest {
                    description: "Invalid Github token for Authorization header.".to_owned(),
                })?,
        );

        let mut hasher = DefaultHasher::new();
        hasher.write(token.as_bytes());

        Ok(Client {
            id: hasher.finish(),
            github_client,
            reqwest_client,
            reqwest_headers,
        })
    }

    /// Get a milestones from the API.
    pub fn get_milestones(&self) -> Result<Vec<Milestone>, Error> {
        Ok(self
            .reqwest_client
            .get("https://api.github.com/repos/reinfer/platform/milestones")
            .headers(self.reqwest_headers.clone())
            .try_send()?)
    }

    /// Assign an issue to a milestone.
    ///
    /// This will overwrite an existing milestone, if present.
    pub fn assign_issue_to_milestone(
        &self,
        issue: &Issue,
        milestone: &Milestone,
    ) -> Result<(), Error> {
        self.reqwest_client
            .patch(&format!(
                "https://api.github.com/repos/reinfer/platform/issues/{}",
                issue.number
            ))
            .json(&IssuePatchMilestone {
                milestone: milestone.number,
            })
            .headers(self.reqwest_headers.clone())
            .send()?;
        Ok(())
    }

    /// Assign an organisation member to an issue.
    ///
    /// This will overwrite any existing assignees, if present.
    pub fn assign_member_to_issue(
        &self,
        member: &OrganisationMember,
        issue: &Issue,
    ) -> Result<(), Error> {
        self.reqwest_client
            .patch(&format!(
                "https://api.github.com/repos/reinfer/platform/issues/{}",
                issue.number
            ))
            .json(&IssuePatchAssignees {
                assignees: vec![member.login.clone()],
            })
            .headers(self.reqwest_headers.clone())
            .send()?;
        Ok(())
    }

    /// Get an issue by number.
    pub fn get_issue_by_number(&self, number: &str) -> Result<Issue, Error> {
        Ok(self
            .github_client
            .get()
            .repos()
            .owner("reinfer")
            .repo("platform")
            .issues()
            .number(&number)
            .try_execute::<Issue>()?)
    }

    /// Get a milestones from the API.
    pub fn get_members(&self) -> Result<Vec<OrganisationMember>, Error> {
        Ok(self
            .github_client
            .get()
            .orgs()
            .org("reinfer")
            .members()
            .try_execute::<Vec<OrganisationMember>>()?)
    }
}

mod error {
    use github_rs::errors::Error as GithubError;
    use reqwest::Error as ReqwestError;
    use snafu::Snafu;

    #[derive(Debug, Snafu)]
    #[snafu(visibility = "pub")]
    pub enum Error {
        #[snafu(display("Bad request to Decadog: {}", description))]
        BadRequest { description: String },

        #[snafu(display("Github client error: {}", source))]
        Github { source: GithubError },

        #[snafu(display("Reqwest error: {}", source))]
        Reqwest { source: ReqwestError },
    }

    impl From<GithubError> for Error {
        fn from(source: GithubError) -> Self {
            Error::Github { source }
        }
    }

    impl From<ReqwestError> for Error {
        fn from(source: ReqwestError) -> Self {
            Error::Reqwest { source }
        }
    }

    // TODO this error cast is very general and should be removed
    // to manual casting if need be
    impl From<String> for Error {
        fn from(source: String) -> Self {
            Error::BadRequest {
                description: source,
            }
        }
    }
}
pub use error::Error;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn invalid_github_token() {
        assert!(Client::new("my_secret_token").is_ok());
        match Client::new("invalid header char -> \n").unwrap_err() {
            Error::BadRequest { description } => assert_eq!(
                description,
                "Invalid Github token for Authorization header."
            ),
            _ => panic!("Unexpected error"),
        }
    }
}
