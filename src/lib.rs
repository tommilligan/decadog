use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::Hasher;

use github_rs::client::{Executor, Github};
use github_rs::StatusCode;
use reqwest::header::{HeaderMap, AUTHORIZATION};
use reqwest::{Client as ReqwestClient, Method, RequestBuilder, Url, UrlError};
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
pub struct Client<'a> {
    id: u64,
    github_client: Github,
    reqwest_client: ReqwestClient,
    reqwest_headers: HeaderMap,

    owner: &'a str,
    repo: &'a str,
    repo_url: Url,
}

impl<'a> fmt::Debug for Client<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Decadog client {}", self.id)
    }
}

impl<'a> PartialEq for Client<'a> {
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

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ClientErrorDetail {
    pub resource: String,
    pub field: String,
    pub code: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ClientErrorBody {
    pub message: String,
    pub errors: Option<Vec<ClientErrorDetail>>,
    pub documentation_url: Option<String>,
}

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
        let mut response = self.send()?;
        let status = response.status();
        if status.is_success() {
            Ok(response.json()?)
        } else if status.is_client_error() {
            Err(Error::Github {
                description: format!("Client error: {:?}", response.json::<ClientErrorBody>()?),
                status,
            })
        } else {
            Err(Error::Github {
                description: format!("Unexpected response status code."),
                status,
            })
        }
    }
}

impl<'a> Client<'a> {
    /// Create a new client that can make requests to the Github API using token auth.
    pub fn new(token: &str, owner: &'a str, repo: &'a str) -> Result<Client<'a>, Error> {
        // Nice github API
        let github_client = Github::new(token)?;

        // Raw REST endpoints
        let reqwest_client = reqwest::Client::new();
        let mut reqwest_headers = HeaderMap::new();
        reqwest_headers.insert(
            AUTHORIZATION,
            format!("token {}", token)
                .parse()
                .map_err(|_| Error::Config {
                    description: "Invalid Github token for Authorization header.".to_owned(),
                })?,
        );

        let repo_url = Url::parse(&format!("https://api.github.com/repos/{}/{}/", owner, repo))
            .map_err(|_| Error::Config {
                description: "Invalid owner or repo name.".to_owned(),
            })?;

        let mut hasher = DefaultHasher::new();
        hasher.write(token.as_bytes());
        hasher.write(owner.as_bytes());
        hasher.write(repo.as_bytes());
        let id = hasher.finish();

        Ok(Client {
            id,
            github_client,
            reqwest_client,
            reqwest_headers,

            owner,
            repo,
            repo_url,
        })
    }

    pub fn owner(&self) -> &str {
        self.owner
    }

    pub fn repo(&self) -> &str {
        self.repo
    }

    pub fn request(&self, method: Method, url: &str) -> Result<RequestBuilder, UrlError> {
        Ok(self
            .reqwest_client
            .request(method, self.repo_url.join(url)?)
            .headers(self.reqwest_headers.clone()))
    }

    /// Get a milestones from the API.
    pub fn get_milestones(&self) -> Result<Vec<Milestone>, Error> {
        Ok(self.request(Method::GET, "milestones")?.try_send()?)
    }

    /// Assign an issue to a milestone.
    ///
    /// This will overwrite an existing milestone, if present.
    pub fn assign_issue_to_milestone(
        &self,
        issue: &Issue,
        milestone: &Milestone,
    ) -> Result<Issue, Error> {
        Ok(self
            .request(Method::PATCH, &format!("issues/{}", issue.number))?
            .json(&IssuePatchMilestone {
                milestone: milestone.number,
            })
            .try_send()?)
    }

    /// Assign an organisation member to an issue.
    ///
    /// This will overwrite any existing assignees, if present.
    pub fn assign_member_to_issue(
        &self,
        member: &OrganisationMember,
        issue: &Issue,
    ) -> Result<Issue, Error> {
        Ok(self
            .request(Method::PATCH, &format!("issues/{}", issue.number))?
            .json(&IssuePatchAssignees {
                assignees: vec![member.login.clone()],
            })
            .try_send()?)
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
    use reqwest::{Error as ReqwestError, StatusCode, UrlError};
    use snafu::Snafu;

    #[derive(Debug, Snafu)]
    #[snafu(visibility = "pub")]
    pub enum Error {
        #[snafu(display("Decadog config error: {}", description))]
        Config { description: String },

        #[snafu(display("Github error: {}", description))]
        Github {
            description: String,
            status: StatusCode,
        },

        #[snafu(display("Reqwest error: {}", source))]
        Reqwest { source: ReqwestError },

        #[snafu(display("Url parse error: {}", source))]
        Url { source: UrlError },

        #[snafu(display("To be removed: {}", description))]
        Old { description: String },
        #[snafu(display("Github client error: {}", source))]
        GithubOld { source: GithubError },
    }

    impl From<GithubError> for Error {
        fn from(source: GithubError) -> Self {
            Error::GithubOld { source }
        }
    }

    impl From<ReqwestError> for Error {
        fn from(source: ReqwestError) -> Self {
            Error::Reqwest { source }
        }
    }

    impl From<UrlError> for Error {
        fn from(source: UrlError) -> Self {
            Error::Url { source }
        }
    }

    // TODO this error cast is very general and should be removed
    // to manual casting if need be
    impl From<String> for Error {
        fn from(source: String) -> Self {
            Error::Old {
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
        assert!(Client::new("my_secret_token", "foo", "bar").is_ok());
        match Client::new("invalid header char -> \n", "foo", "bar").unwrap_err() {
            Error::Config { description } => assert_eq!(
                description,
                "Invalid Github token for Authorization header."
            ),
            _ => panic!("Unexpected error"),
        }
    }

    #[test]
    fn client_equality_by_args() {
        assert!(
            Client::new("my_secret_token", "foo", "bar").unwrap()
                == Client::new("my_secret_token", "foo", "bar").unwrap()
        );
        assert!(
            Client::new("my_secret_token", "foo", "bar").unwrap()
                != Client::new("other", "foo", "bar").unwrap()
        );
        assert!(
            Client::new("my_secret_token", "foo", "bar").unwrap()
                != Client::new("my_secret_token", "other", "bar").unwrap()
        );
        assert!(
            Client::new("my_secret_token", "foo", "bar").unwrap()
                != Client::new("my_secret_token", "foo", "other").unwrap()
        );
    }
}
