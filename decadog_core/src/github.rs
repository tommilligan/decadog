/// Github integration.
use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::Hasher;

use chrono::{DateTime, FixedOffset};
use log::{debug, error};
use reqwest::header::{HeaderMap, AUTHORIZATION};
use reqwest::{Client as ReqwestClient, Method, RequestBuilder, Url};
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
        write!(f, "Github client {}", self.id)
    }
}

/// Detail of a single client error.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GithubClientErrorDetail {
    pub resource: String,
    pub field: String,
    pub code: String,
}

/// Returned from the API when one or more client errors have been made.
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
            Err(Error::Github {
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

impl Client {
    /// Create a new client that can make requests to the Github API using token auth.
    pub fn new(url: &str, token: &str) -> Result<Client, Error> {
        // Create reqwest client to interact with APIs
        // TODO: should we pass in an external client here?
        let reqwest_client = reqwest::Client::new();

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            format!("token {}", token)
                .parse()
                .map_err(|_| Error::Config {
                    description: "Invalid Github token for Authorization header.".to_owned(),
                })?,
        );

        let base_url = Url::parse(url).map_err(|_| Error::Config {
            description: format!("Invalid Github base url {}", url),
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

    /// Returns a `request::RequestBuilder` authorized to the Github API.
    pub fn request(&self, method: Method, url: Url) -> RequestBuilder {
        debug!("{} {}", method, url.as_str());
        self.reqwest_client
            .request(method, url)
            .headers(self.headers.clone())
    }

    /// Get an issue by owner, repo name and issue number.
    pub fn get_issue(&self, owner: &str, repo: &str, issue_number: u32) -> Result<Issue, Error> {
        self.request(
            Method::GET,
            self.base_url.join(&format!(
                "/repos/{}/{}/issues/{}",
                owner, repo, issue_number
            ))?,
        )
        .send_github()
    }

    /// Get a repository by owner and repo name.
    pub fn get_repository(&self, owner: &str, repo: &str) -> Result<Repository, Error> {
        self.request(
            Method::GET,
            self.base_url.join(&format!("/repos/{}/{}", owner, repo))?,
        )
        .send_github()
    }

    /// Get members by organisation.
    pub fn get_members(&self, organisation: &str) -> Result<Vec<OrganisationMember>, Error> {
        self.request(
            Method::GET,
            self.base_url
                .join(&format!("orgs/{}/members", organisation))?,
        )
        .send_github()
    }

    /// Get milestones by owner and repo name.
    pub fn get_milestones(&self, owner: &str, repo: &str) -> Result<Vec<Milestone>, Error> {
        let query = GetMilestones {
            state: None,
            sort: None,
            direction: Some(Direction::Descending),
        };
        self.request(
            Method::GET,
            self.base_url
                .join(&format!("/repos/{}/{}/milestones", owner, repo))?,
        )
        .query(&query)
        .send_github()
    }

    /// Update issue.
    pub fn patch_issue(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u32,
        update: &IssueUpdate,
    ) -> Result<Issue, Error> {
        self.request(
            Method::PATCH,
            self.base_url.join(&format!(
                "/repos/{}/{}/issues/{}",
                owner, repo, issue_number
            ))?,
        )
        .json(update)
        .send_github()
    }

    /// Search issues.
    pub fn search_issues(&self, query: &SearchIssues) -> Result<Vec<Issue>, Error> {
        let builder = self
            .request(Method::GET, self.base_url.join("search/issues")?)
            .query(&query);

        let results: GithubSearchResults<Issue> = builder.send_github()?;
        if results.incomplete_results {
            // FIXME handle github pagination
            error!("Incomplete results recieved from Github Search API, this is bad");
        }
        Ok(results.items)
    }

    pub fn patch_milestone(
        &self,
        owner: &str,
        repo: &str,
        milestone_number: u32,
        update: &MilestoneUpdate,
    ) -> Result<Milestone, Error> {
        self.request(
            Method::PATCH,
            self.base_url.join(&format!(
                "/repos/{}/{}/milestones/{}",
                owner, repo, milestone_number
            ))?,
        )
        .json(update)
        .send_github()
    }
}

/// Update an issue.
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct IssueUpdate {
    /// Assign milestone. Use `None` to skip assignment, Some(None) to clear.
    #[allow(clippy::option_option)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub milestone: Option<Option<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignees: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<State>,
}

/// A search filter for state.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SearchState {
    Open,
    Closed,
    All,
}

/// Direction in which to return results.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub enum Direction {
    #[serde(rename = "asc")]
    Ascending,
    #[serde(rename = "desc")]
    Descending,
}

/// Request to search issues.
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct SearchIssues {
    pub q: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
    /// Ignored unless `sort` is provided.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<Direction>,
}

/// Request to get milestones.
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct GetMilestones {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<SearchState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<Direction>,
}

/// A Github Milestone.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct Milestone {
    pub id: u32,
    pub number: u32,
    pub title: String,
    pub state: State,
    pub due_on: DateTime<FixedOffset>,
}

/// Update a milestone.
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct MilestoneUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// A memeber reference in an Organisation.
#[derive(Deserialize, Serialize, Debug, Clone, Default, PartialEq)]
pub struct OrganisationMember {
    pub login: String,
    pub id: u32,
}

/// A Github User.
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct User {
    pub login: String,
    pub id: u32,
    pub name: String,
}

/// A Github status.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Open,
    Closed,
}

impl Default for State {
    fn default() -> Self {
        Self::Open
    }
}

/// A Github Issue.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct Issue {
    pub id: u32,
    pub number: u32,
    pub state: State,
    pub title: String,
    pub milestone: Option<Milestone>,
    pub assignees: Vec<OrganisationMember>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
    pub closed_at: Option<DateTime<FixedOffset>>,
}

/// A Github Repository.
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct Repository {
    pub id: u64,
    pub name: String,
}

impl fmt::Display for Milestone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.title)
    }
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.number, self.title)
    }
}

/// A response from the Github search API.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct GithubSearchResults<T> {
    pub incomplete_results: bool,
    pub items: Vec<T>,
}

#[cfg(test)]
pub mod tests {
    use chrono::{FixedOffset, NaiveDate, TimeZone};
    use lazy_static::lazy_static;
    use mockito::mock;
    use pretty_assertions::assert_eq;

    use super::*;

    const MOCK_GITHUB_TOKEN: &str = "mock_token";
    lazy_static! {
        pub static ref MOCK_GITHUB_CLIENT: Client =
            Client::new(&mockito::server_url(), MOCK_GITHUB_TOKEN)
                .expect("Couldn't create mock github client");
    }

    #[test]
    fn invalid_github_token() {
        assert!(Client::new("https://api.mygithub.com/", "github_token").is_ok());
        match Client::new("https://api.mygithub.com/", "invalid header char -> \n").unwrap_err() {
            Error::Config { description } => assert_eq!(
                description,
                "Invalid Github token for Authorization header."
            ),
            _ => panic!("Unexpected error"),
        }
    }

    #[test]
    fn test_get_issue() {
        let body = r#"{
  "id": 1234567,
  "number": 1,
  "state": "open",
  "title": "Mock Title",
  "body": "Mock description",
  "assignees": [
    {
      "login": "tommilligan",
      "id": 1
    }
  ],
  "milestone": {
    "id": 1002604,
    "number": 1,
    "state": "open",
    "title": "v1.0",
    "due_on": "2012-10-09T23:39:01Z"
  },
  "created_at": "2011-04-22T13:33:48Z",
  "updated_at": "2011-04-22T13:33:48Z"
}"#;
        let mock = mock("GET", "/repos/tommilligan/decadog/issues/1")
            .match_header("authorization", "token mock_token")
            .with_status(200)
            .with_body(body)
            .create();

        let issue = MOCK_GITHUB_CLIENT
            .get_issue("tommilligan", "decadog", 1)
            .unwrap();
        mock.assert();

        assert_eq!(
            issue,
            Issue {
                id: 1_234_567,
                number: 1,
                state: State::Open,
                title: "Mock Title".to_owned(),
                milestone: Some(Milestone {
                    id: 1_002_604,
                    number: 1,
                    title: "v1.0".to_owned(),
                    state: State::Open,
                    due_on: FixedOffset::east(0)
                        .from_utc_datetime(&NaiveDate::from_ymd(2012, 10, 9).and_hms(23, 39, 1)),
                }),
                assignees: vec![OrganisationMember {
                    login: "tommilligan".to_owned(),
                    id: 1
                }],
                created_at: FixedOffset::east(0)
                    .from_utc_datetime(&NaiveDate::from_ymd(2011, 4, 22).and_hms(13, 33, 48)),
                updated_at: FixedOffset::east(0)
                    .from_utc_datetime(&NaiveDate::from_ymd(2011, 4, 22).and_hms(13, 33, 48)),
                closed_at: None,
            }
        );
    }

    #[test]
    fn test_close_issue() {
        let body = r#"{
  "id": 1234567,
  "number": 1,
  "state": "closed",
  "title": "Mock Title",
  "body": "Mock description",
  "assignees": [],
  "milestone": null,
  "created_at": "2011-04-22T13:33:48Z",
  "updated_at": "2011-04-22T13:33:48Z"
}"#;
        let mock = mock("PATCH", "/repos/tommilligan/decadog/issues/1")
            .match_header("authorization", "token mock_token")
            .match_body(r#"{"state":"closed"}"#)
            .with_status(200)
            .with_body(body)
            .create();

        let mut update = IssueUpdate::default();
        update.state = Some(State::Closed);
        let issue = MOCK_GITHUB_CLIENT
            .patch_issue("tommilligan", "decadog", 1, &update)
            .unwrap();
        mock.assert();

        assert_eq!(
            issue,
            Issue {
                id: 1_234_567,
                number: 1,
                state: State::Closed,
                title: "Mock Title".to_owned(),
                milestone: None,
                assignees: vec![],
                created_at: FixedOffset::east(0)
                    .from_utc_datetime(&NaiveDate::from_ymd(2011, 4, 22).and_hms(13, 33, 48)),
                updated_at: FixedOffset::east(0)
                    .from_utc_datetime(&NaiveDate::from_ymd(2011, 4, 22).and_hms(13, 33, 48)),
                closed_at: None,
            }
        );
    }
}
