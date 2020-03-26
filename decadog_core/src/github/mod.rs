/// Github integration.
use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::Hasher;

use chrono::{DateTime, FixedOffset, TimeZone};
use log::debug;
use reqwest::header::{HeaderMap, AUTHORIZATION};
use reqwest::{
    blocking::{Client as ReqwestClient, ClientBuilder, RequestBuilder},
    Method, Url,
};
use serde_derive::{Deserialize, Serialize};

use crate::error::Error;

pub mod paginate;
pub mod request;

use paginate::PaginatedSearch;
use request::RequestBuilderExt;

pub struct Client {
    id: u64,
    reqwest_client: ReqwestClient,
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

impl Client {
    /// Create a new client that can make requests to the Github API using token auth.
    pub fn new(url: &str, token: &str) -> Result<Client, Error> {
        // Create reqwest client to interact with APIs
        // TODO: should we pass in an external client here?
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            format!("token {}", token)
                .parse()
                .map_err(|_| Error::Config {
                    description: "Invalid Github token for Authorization header.".to_owned(),
                })?,
        );

        let reqwest_client = ClientBuilder::new()
            .default_headers(headers)
            .user_agent("decadog")
            .build()?;

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
            base_url,
        })
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    /// Returns a `request::RequestBuilder` authorized to the Github API.
    pub fn request(&self, method: Method, url: Url) -> RequestBuilder {
        debug!("{} {}", method, url.as_str());
        self.reqwest_client.request(method, url)
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

    /// Get milestones by owner and repo name.
    pub fn create_milestone(
        &self,
        owner: &str,
        repo: &str,
        create: &MilestoneUpdate,
    ) -> Result<Milestone, Error> {
        self.request(
            Method::POST,
            self.base_url
                .join(&format!("/repos/{}/{}/milestones", owner, repo))?,
        )
        .json(&create)
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
    pub fn search_issues(&self, query: &SearchIssues) -> Result<PaginatedSearch<Issue>, Error> {
        let builder = self
            .request(Method::GET, self.base_url.join("search/issues")?)
            .query(&query);
        let request = builder.build()?;

        PaginatedSearch::<Issue>::new(&self.reqwest_client, request)
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

#[derive(Debug, Default, Clone, PartialEq)]
pub struct SearchQueryBuilder {
    query: String,
}

impl SearchQueryBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn build(self) -> String {
        self.query
    }

    fn push_separator(&mut self) {
        if !self.query.is_empty() {
            self.query.push_str(" ");
        };
    }

    pub fn term(mut self, term: &str) -> Self {
        self.push_separator();
        self.query.push_str(term);
        self
    }

    pub fn key_value(mut self, key: &str, value: &str) -> Self {
        self.push_separator();
        self.query.push_str(key);
        self.query.push_str(":");
        self.query.push_str(value);
        self
    }

    pub fn label(self, label_name: &str) -> Self {
        self.key_value("label", label_name)
    }

    pub fn not_label(self, label_name: &str) -> Self {
        self.key_value("-label", label_name)
    }

    pub fn issue(self) -> Self {
        self.key_value("type", "issue")
    }

    pub fn state(self, state: &State) -> Self {
        self.key_value(
            "state",
            &serde_plain::to_string(state).expect("Serializing state to string failed"),
        )
    }

    pub fn milestone(self, milestone_title: &str) -> Self {
        self.term(&format!(r#"milestone:"{}""#, milestone_title))
    }

    pub fn closed_on_or_after<Tz: TimeZone>(self, datetime: &DateTime<Tz>) -> Self
    where
        Tz::Offset: fmt::Display,
    {
        self.state(&State::Closed)
            .term(&format!("closed:>={}", &datetime.format("%Y-%m-%d")))
    }

    pub fn owner_repo(self, owner: &str, repo: &str) -> Self {
        self.term(&format!("repo:{}/{}", owner, repo))
    }
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_page: Option<u32>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<State>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_on: Option<DateTime<FixedOffset>>,
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

/// A Github label.
#[derive(Deserialize, Serialize, Debug, Clone, Default, PartialEq)]
pub struct Label {
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
    pub labels: Vec<Label>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
    pub closed_at: Option<DateTime<FixedOffset>>,
    pub html_url: String,
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
    fn search_query_builder() {
        let q = SearchQueryBuilder::new();
        assert_eq!(&q.clone().build(), "");
        assert_eq!(&q.clone().state(&State::Open).build(), "state:open");
        assert_eq!(
            &q.clone().issue().label("spam").build(),
            "type:issue label:spam"
        );
        assert_eq!(
            &q.clone().milestone("Sprint 2").not_label("spam").build(),
            r#"milestone:"Sprint 2" -label:spam"#
        );
        assert_eq!(
            &q.clone().term("arbitrary").key_value("k", "v").build(),
            r#"arbitrary k:v"#
        );
        assert_eq!(
            &q.clone()
                .closed_on_or_after(
                    &FixedOffset::east(0)
                        .from_utc_datetime(&NaiveDate::from_ymd(2011, 4, 22).and_hms(13, 33, 48)),
                )
                .owner_repo("ow", "re")
                .build(),
            r#"state:closed closed:>=2011-04-22 repo:ow/re"#
        );
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
  "labels": [
    {
      "id": 248,
      "name": "taggy"
    }
  ],
  "created_at": "2011-04-22T13:33:48Z",
  "updated_at": "2011-04-22T13:33:48Z",
  "html_url": "http://foo.bar"
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
                labels: vec![Label {
                    id: 248,
                    name: "taggy".to_owned()
                }],
                created_at: FixedOffset::east(0)
                    .from_utc_datetime(&NaiveDate::from_ymd(2011, 4, 22).and_hms(13, 33, 48)),
                updated_at: FixedOffset::east(0)
                    .from_utc_datetime(&NaiveDate::from_ymd(2011, 4, 22).and_hms(13, 33, 48)),
                closed_at: None,
                html_url: "http://foo.bar".to_owned(),
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
  "labels": [],
  "created_at": "2011-04-22T13:33:48Z",
  "updated_at": "2011-04-22T13:33:48Z",
  "html_url": "http://foo.bar"
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
                labels: vec![],
                created_at: FixedOffset::east(0)
                    .from_utc_datetime(&NaiveDate::from_ymd(2011, 4, 22).and_hms(13, 33, 48)),
                updated_at: FixedOffset::east(0)
                    .from_utc_datetime(&NaiveDate::from_ymd(2011, 4, 22).and_hms(13, 33, 48)),
                closed_at: None,
                html_url: "http://foo.bar".to_owned(),
            }
        );
    }

    #[test]
    fn test_close_milestone() {
        let body = r#"{
  "id": 1234567,
  "number": 1,
  "state": "closed",
  "title": "Mock Title",
  "due_on": "2011-04-22T13:33:48Z"
}"#;
        let mock = mock("PATCH", "/repos/tommilligan/decadog/milestones/1")
            .match_header("authorization", "token mock_token")
            .match_body(r#"{"state":"closed"}"#)
            .with_status(200)
            .with_body(body)
            .create();

        let mut update = MilestoneUpdate::default();
        update.state = Some(State::Closed);
        let milestone = MOCK_GITHUB_CLIENT
            .patch_milestone("tommilligan", "decadog", 1, &update)
            .unwrap();
        mock.assert();

        assert_eq!(
            milestone,
            Milestone {
                id: 1_234_567,
                number: 1,
                state: State::Closed,
                title: "Mock Title".to_owned(),
                due_on: FixedOffset::east(0)
                    .from_utc_datetime(&NaiveDate::from_ymd(2011, 4, 22).and_hms(13, 33, 48)),
            }
        );
    }
}
