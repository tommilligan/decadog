use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::Hasher;

use reqwest::header::{HeaderMap, AUTHORIZATION};
use reqwest::{Client as ReqwestClient, IntoUrl, Method, RequestBuilder, Url, UrlError};
use serde::de::DeserializeOwned;
use serde_derive::{Deserialize, Serialize};

mod core;

pub use crate::core::{AssignedTo, Board, Issue, Milestone, OrganisationMember, Repository};

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
    reqwest_client: ReqwestClient,
    github_headers: HeaderMap,
    zenhub_headers: HeaderMap,

    owner: &'a str,
    repo: &'a str,
    repo_url: Url,
    zenhub_url: Url,
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

struct Github;
struct Zenhub;

/// Send a HTTP request to an API, and return the resulting struct.
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
    pub fn new(
        owner: &'a str,
        repo: &'a str,
        github_token: &str,
        zenhub_token: &str,
    ) -> Result<Client<'a>, Error> {
        // Create reqwest client to interact with APIs
        let reqwest_client = reqwest::Client::new();

        let mut github_headers = HeaderMap::new();
        github_headers.insert(
            AUTHORIZATION,
            format!("token {}", github_token)
                .parse()
                .map_err(|_| Error::Config {
                    description: "Invalid Github token for Authorization header.".to_owned(),
                })?,
        );

        let mut zenhub_headers = HeaderMap::new();
        zenhub_headers.insert(
            "x-authentication-token",
            zenhub_token.parse().map_err(|_| Error::Config {
                description: "Invalid Zenhub token for Authorization header.".to_owned(),
            })?,
        );

        let repo_url = Url::parse(&format!("https://api.github.com/repos/{}/{}/", owner, repo))
            .map_err(|_| Error::Config {
                description: "Invalid owner or repo name.".to_owned(),
            })?;
        let zenhub_url = Url::parse("https://api.zenhub.io/")?;

        let mut hasher = DefaultHasher::new();
        hasher.write(owner.as_bytes());
        hasher.write(repo.as_bytes());
        hasher.write(github_token.as_bytes());
        hasher.write(zenhub_token.as_bytes());
        let id = hasher.finish();

        Ok(Client {
            id,
            reqwest_client,
            github_headers,
            zenhub_headers,

            owner,
            repo,
            repo_url,
            zenhub_url,
        })
    }

    pub fn owner(&self) -> &str {
        self.owner
    }

    pub fn repo(&self) -> &str {
        self.repo
    }

    pub fn github<U: IntoUrl>(&self, method: Method, url: U) -> Result<RequestBuilder, UrlError> {
        Ok(self
            .reqwest_client
            .request(method, url)
            .headers(self.github_headers.clone()))
    }

    pub fn zenhub<U: IntoUrl>(&self, method: Method, url: U) -> Result<RequestBuilder, UrlError> {
        Ok(self
            .reqwest_client
            .request(method, url)
            .headers(self.zenhub_headers.clone()))
    }

    /// Get Zenhub board for a repository.
    pub fn get_board(&self, repository: &Repository) -> Result<Board, Error> {
        Ok(self
            .zenhub(
                Method::GET,
                self.zenhub_url
                    .join(&format!("/p1/repositories/{}/board", repository.id))?,
            )?
            .try_send()?)
    }

    /// Get a repository from the API.
    pub fn get_repository(&self) -> Result<Repository, Error> {
        Ok(self
            .github(
                Method::GET,
                Url::parse(&format!(
                    "https://api.github.com/repos/{}/{}",
                    self.owner, self.repo
                ))?,
            )?
            .try_send()?)
    }

    /// Get a milestones from the API.
    pub fn get_milestones(&self) -> Result<Vec<Milestone>, Error> {
        Ok(self
            .github(Method::GET, self.repo_url.join("milestones")?)?
            .try_send()?)
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
            .github(
                Method::PATCH,
                self.repo_url.join(&format!("issues/{}", issue.number))?,
            )?
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
            .github(
                Method::PATCH,
                self.repo_url.join(&format!("issues/{}", issue.number))?,
            )?
            .json(&IssuePatchAssignees {
                assignees: vec![member.login.clone()],
            })
            .try_send()?)
    }

    /// Get an issue by number.
    pub fn get_issue_by_number(&self, number: &str) -> Result<Issue, Error> {
        Ok(self
            .github(
                Method::GET,
                self.repo_url.join(&format!("issues/{}", number))?,
            )?
            .try_send()?)
    }

    /// Get a milestones from the API.
    pub fn get_members(&self) -> Result<Vec<OrganisationMember>, Error> {
        Ok(self
            .github(
                Method::GET,
                &format!("https://api.github.com/orgs/{}/members", self.owner),
            )?
            .try_send()?)
    }
}

mod error {
    use reqwest::{Error as ReqwestError, StatusCode, UrlError};
    use snafu::Snafu;

    #[derive(Debug, Snafu)]
    #[snafu(visibility = "pub")]
    pub enum Error {
        #[snafu(display("Decadog config error: {}", description))]
        Config { description: String },

        #[snafu(display("Github error [{}]: {}", status, description))]
        Github {
            description: String,
            status: StatusCode,
        },

        #[snafu(display("Reqwest error: {}", source))]
        Reqwest { source: ReqwestError },

        #[snafu(display("Url parse error: {}", source))]
        Url { source: UrlError },
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
}
pub use error::Error;

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn invalid_github_token() {
        assert!(Client::new("foo", "bar", "github_token", "zenhub_token").is_ok());
        match Client::new("foo", "bar", "invalid header char -> \n", "zenhub_token").unwrap_err() {
            Error::Config { description } => assert_eq!(
                description,
                "Invalid Github token for Authorization header."
            ),
            _ => panic!("Unexpected error"),
        }
    }

    #[test]
    fn client_equality_by_args() {
        assert_eq!(
            Client::new("foo", "bar", "github_token", "zenhub_token").unwrap(),
            Client::new("foo", "bar", "github_token", "zenhub_token").unwrap(),
        );
        assert_ne!(
            Client::new("foo", "bar", "github_token", "zenhub_token").unwrap(),
            Client::new("foo", "other", "github_token", "zenhub_token").unwrap(),
        );
        assert_ne!(
            Client::new("foo", "bar", "github_token", "zenhub_token").unwrap(),
            Client::new("foo", "bar", "github_token", "other").unwrap(),
        );
    }
}
