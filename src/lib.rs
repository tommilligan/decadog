use github_rs::client::{Executor, Github};
use github_rs::StatusCode;
use reqwest::header::{HeaderMap, AUTHORIZATION};
use reqwest::Client as ReqwestClient;
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

pub struct Client {
    github_client: Github,
    reqwest_client: ReqwestClient,
    reqwest_headers: HeaderMap,
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
                .expect("Invalid auth header"),
        );

        Ok(Client {
            github_client,
            reqwest_client,
            reqwest_headers,
        })
    }

    /// Get a milestones from the API.
    pub fn get_milestones(&self) -> Vec<Milestone> {
        self.reqwest_client
            .get("https://api.github.com/repos/reinfer/platform/milestones")
            .headers(self.reqwest_headers.clone())
            .send()
            .unwrap()
            .json()
            .unwrap()
    }

    /// Assign an issue to a milestone.
    ///
    /// This will overwrite an existing milestone, if present.
    pub fn assign_issue_to_milestone(&self, issue: &Issue, milestone: &Milestone) -> () {
        self.reqwest_client
            .patch(&format!(
                "https://api.github.com/repos/reinfer/platform/issues/{}",
                issue.number
            ))
            .json(&IssuePatchMilestone {
                milestone: milestone.number,
            })
            .headers(self.reqwest_headers.clone())
            .send()
            .unwrap();
    }

    /// Assign an organisation member to an issue.
    ///
    /// This will overwrite any existing assignees, if present.
    pub fn assign_member_to_issue(&self, member: &OrganisationMember, issue: &Issue) -> () {
        self.reqwest_client
            .patch(&format!(
                "https://api.github.com/repos/reinfer/platform/issues/{}",
                issue.number
            ))
            .json(&IssuePatchAssignees {
                assignees: vec![member.login.clone()],
            })
            .headers(self.reqwest_headers.clone())
            .send()
            .unwrap();
    }

    /// Get an issue by number.
    pub fn get_issue_by_number(&self, number: &str) -> Issue {
        self.github_client
            .get()
            .repos()
            .owner("reinfer")
            .repo("platform")
            .issues()
            .number(&number)
            .try_execute::<Issue>()
            .expect("Failed to get issue")
    }

    /// Get a milestones from the API.
    pub fn get_members(&self) -> Vec<OrganisationMember> {
        self.github_client
            .get()
            .orgs()
            .org("reinfer")
            .members()
            .try_execute::<Vec<OrganisationMember>>()
            .expect("Failed to get users")
    }
}

mod error {
    use github_rs::errors::Error as GithubError;
    use snafu::Snafu;

    #[derive(Debug, Snafu)]
    #[snafu(visibility = "pub")]
    pub enum Error {
        #[snafu(display("Github client error: {}", source))]
        Github { source: GithubError },

        #[snafu(display("Protocol error: {}", reason))]
        Protocol { reason: String },
    }

    impl From<GithubError> for Error {
        fn from(source: GithubError) -> Self {
            Error::Github { source }
        }
    }
}
pub use error::Error;

pub type Result<T, E = Error> = std::result::Result<T, E>;
