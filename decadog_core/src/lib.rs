#![deny(clippy::all)]

use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::Hasher;

use chrono::{DateTime, FixedOffset};
use log::debug;
use reqwest::header::HeaderMap;
use reqwest::{Client as ReqwestClient, Method, RequestBuilder, Url, UrlError};
use serde::de::DeserializeOwned;
use serde_derive::{Deserialize, Serialize};

mod core;
pub mod github;
pub mod secret;

pub use crate::core::{
    AssignedTo, Board, Issue, Milestone, OrganisationMember, Pipeline, PipelinePosition,
    Repository, SetEstimate, Sprint, StartDate, ZenhubIssue,
};

use github::{IssueUpdate, SearchIssues};

/// Decadog client, used to abstract complex tasks over the Github API.
pub struct Client<'a> {
    id: u64,
    reqwest_client: ReqwestClient,
    zenhub_headers: HeaderMap,

    owner: &'a str,
    repo: &'a str,
    zenhub_url: Url,

    github: &'a github::Client,
}

impl<'a> fmt::Debug for Client<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Decadog client {}", self.id)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GithubClientErrorDetail {
    pub resource: String,
    pub field: String,
    pub code: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GithubClientErrorBody {
    pub message: String,
    pub errors: Option<Vec<GithubClientErrorDetail>>,
    pub documentation_url: Option<String>,
}

/// Send a HTTP request to Github, and return the resulting struct.
trait SendGithubExt {
    fn send_github<T>(self) -> Result<T, Error>
    where
        Self: Sized,
        T: DeserializeOwned;
}

impl SendGithubExt for RequestBuilder {
    fn send_github<T>(self) -> Result<T, Error>
    where
        Self: Sized,
        T: DeserializeOwned,
    {
        let mut response = self.send()?;
        let status = response.status();
        if status.is_success() {
            Ok(response.json()?)
        } else if status.is_client_error() {
            Err(Error::GithubClient {
                error: response.json()?,
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

/// Send a HTTP request to Github, and return the resulting struct.
trait SendZenhubExt {
    fn send_zenhub<T>(self) -> Result<T, Error>
    where
        Self: Sized,
        T: DeserializeOwned;

    fn send_zenhub_no_response(self) -> Result<(), Error>
    where
        Self: Sized;
}

/// Send a HTTP request to Github, and return the resulting struct.
impl SendZenhubExt for RequestBuilder {
    fn send_zenhub<T>(self) -> Result<T, Error>
    where
        Self: Sized,
        T: DeserializeOwned,
    {
        let mut response = self.send()?;
        let status = response.status();
        if status.is_success() {
            Ok(response.json()?)
        } else if status.is_client_error() {
            Err(Error::ZenhubClient {
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

    fn send_zenhub_no_response(self) -> Result<(), Error>
    where
        Self: Sized,
    {
        let mut response = self.send()?;
        let status = response.status();
        if status.is_success() {
            Ok(())
        } else if status.is_client_error() {
            Err(Error::ZenhubClient {
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

impl<'a> Client<'a> {
    /// Create a new client that can make requests to the Github API using token auth.
    pub fn new(
        owner: &'a str,
        repo: &'a str,
        zenhub_token: &'a str,
        github: &'a github::Client,
    ) -> Result<Client<'a>, Error> {
        // Create reqwest client to interact with APIs
        let reqwest_client = reqwest::Client::new();

        let mut zenhub_headers = HeaderMap::new();
        zenhub_headers.insert(
            "x-authentication-token",
            zenhub_token.parse().map_err(|_| Error::Config {
                description: "Invalid Zenhub token for Authorization header.".to_owned(),
            })?,
        );

        let zenhub_url = Url::parse("https://api.zenhub.io/")?;

        let mut hasher = DefaultHasher::new();
        hasher.write(owner.as_bytes());
        hasher.write(repo.as_bytes());
        hasher.write(zenhub_token.as_bytes());
        let id = hasher.finish();

        Ok(Client {
            id,
            reqwest_client,
            zenhub_headers,

            owner,
            repo,
            zenhub_url,

            github,
        })
    }

    pub fn owner(&self) -> &str {
        self.owner
    }

    pub fn repo(&self) -> &str {
        self.repo
    }

    pub fn zenhub(&self, method: Method, url: Url) -> Result<RequestBuilder, UrlError> {
        debug!("{} {}", method, url.as_str());
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
            .send_zenhub()?)
    }

    /// Get Zenhub StartDate for a Github Milestone.
    pub fn get_start_date(
        &self,
        repository: &Repository,
        milestone: &Milestone,
    ) -> Result<StartDate, Error> {
        Ok(self
            .zenhub(
                Method::GET,
                self.zenhub_url.join(&format!(
                    "/p1/repositories/{}/milestones/{}/start_date",
                    repository.id, milestone.number
                ))?,
            )?
            .send_zenhub()?)
    }

    /// Get Zenhub issue metadata.
    pub fn get_zenhub_issue(
        &self,
        repository: &Repository,
        issue: &Issue,
    ) -> Result<ZenhubIssue, Error> {
        Ok(self
            .zenhub(
                Method::GET,
                self.zenhub_url.join(&format!(
                    "/p1/repositories/{}/issues/{}",
                    repository.id, issue.number
                ))?,
            )?
            .send_zenhub()?)
    }

    /// Set Zenhub issue estimate.
    pub fn set_estimate(
        &self,
        repository: &Repository,
        issue: &Issue,
        estimate: u32,
    ) -> Result<(), Error> {
        Ok(self
            .zenhub(
                Method::PUT,
                self.zenhub_url.join(&format!(
                    "/p1/repositories/{}/issues/{}/estimate",
                    repository.id, issue.number
                ))?,
            )?
            .json(&SetEstimate::from(estimate))
            .send_zenhub_no_response()?)
    }

    /// Get sprint for milestone.
    pub fn get_sprint<'b>(
        &self,
        repository: &Repository,
        milestone: &'b Milestone,
    ) -> Result<Sprint<'b>, Error> {
        let start_date = self.get_start_date(repository, milestone)?;
        Ok(Sprint {
            milestone,
            start_date,
        })
    }

    /// Move issue to a Zenhub pipeline.
    pub fn move_issue(
        &self,
        repository: &Repository,
        issue: &Issue,
        position: &PipelinePosition,
    ) -> Result<(), Error> {
        Ok(self
            .zenhub(
                Method::POST,
                self.zenhub_url.join(&format!(
                    "/p1/repositories/{}/issues/{}/moves",
                    repository.id, issue.number
                ))?,
            )?
            .json(position)
            .send_zenhub_no_response()?)
    }

    /// Get a repository from the API.
    pub fn get_repository(&self) -> Result<Repository, Error> {
        self.github.get_repository(self.owner, self.repo)
    }

    /// Get an issue from the API.
    pub fn get_issue(&self, issue_number: u32) -> Result<Issue, Error> {
        self.github.get_issue(self.owner, self.repo, issue_number)
    }

    /// Get milestones from the API.
    pub fn get_milestones(&self) -> Result<Vec<Milestone>, Error> {
        self.github.get_milestones(self.owner, self.repo)
    }

    /// Assign an issue to a milestone.
    ///
    /// This will overwrite an existing milestone, if present.
    pub fn assign_issue_to_milestone(
        &self,
        issue: &Issue,
        milestone: &Milestone,
    ) -> Result<Issue, Error> {
        let mut update = IssueUpdate::default();
        update.milestone = Some(milestone.number);

        self.github
            .patch_issue(&self.owner, &self.repo, issue.number, &update)
    }

    /// Assign an organisation member to an issue.
    ///
    /// This will overwrite any existing assignees, if present.
    pub fn assign_member_to_issue(
        &self,
        member: &OrganisationMember,
        issue: &Issue,
    ) -> Result<Issue, Error> {
        let mut update = IssueUpdate::default();
        update.assignees = Some(vec![member.login.clone()]);

        self.github
            .patch_issue(&self.owner, &self.repo, issue.number, &update)
    }

    /// Get issues closed after the given datetime.
    pub fn get_issues_closed_after(
        &self,
        datetime: &DateTime<FixedOffset>,
    ) -> Result<Vec<Issue>, Error> {
        let query = SearchIssues {
            q: format!(
                "repo:{}/{} type:issue state:closed closed:>={}",
                self.owner,
                self.repo,
                datetime.format("%Y-%m-%d")
            ),
            sort: Some("updated".to_owned()),
            order: Some("asc".to_owned()),
        };
        self.github.search_issues(&query)
    }

    /// Get issues open in a given milestone.
    pub fn get_milestone_open_issues(&self, milestone: &Milestone) -> Result<Vec<Issue>, Error> {
        let query = SearchIssues {
            q: format!(
                r#"repo:{}/{} type:issue state:open milestone:"{}""#,
                self.owner, self.repo, milestone.title
            ),
            sort: Some("updated".to_owned()),
            order: Some("asc".to_owned()),
        };
        self.github.search_issues(&query)
    }

    /// Get organisation members.
    pub fn get_members(&self) -> Result<Vec<OrganisationMember>, Error> {
        self.github.get_members(self.owner)
    }
}

mod error {
    use reqwest::{Error as ReqwestError, StatusCode, UrlError};
    use snafu::Snafu;

    use crate::GithubClientErrorBody;

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
        GithubClient {
            error: GithubClientErrorBody,
            status: StatusCode,
        },

        #[snafu(display("Reqwest error: {}", source))]
        Reqwest { source: ReqwestError },

        #[snafu(display("Url parse error: {}", source))]
        Url { source: UrlError },

        #[snafu(display("Zenhub error [{}]: {}", status, description))]
        ZenhubClient {
            description: String,
            status: StatusCode,
        },
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
