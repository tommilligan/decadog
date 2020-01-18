use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::Hasher;
use std::ops::Not;

use chrono::{DateTime, FixedOffset};
use log::debug;
use reqwest::header::HeaderMap;
use reqwest::{Client as ReqwestClient, Method, RequestBuilder, Url};
use serde::de::DeserializeOwned;
use serde::ser::Serializer;
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
    pub fn request(&self, method: Method, url: Url) -> RequestBuilder {
        debug!("{} {}", method, url.as_str());
        self.reqwest_client
            .request(method, url)
            .headers(self.headers.clone())
    }

    /// Get the first Zenhub workspace for a repository.
    pub fn get_first_workspace(&self, repository_id: u64) -> Result<Workspace, Error> {
        self.get_workspaces(repository_id)?
            .into_iter()
            .nth(0)
            .ok_or_else(|| Error::Unknown {
                description: "No Zenhub workspace found for repository.".to_owned(),
            })
    }

    /// Get Zenhub workspaces for a repository.
    pub fn get_workspaces(&self, repository_id: u64) -> Result<Vec<Workspace>, Error> {
        self.request(
            Method::GET,
            self.base_url
                .join(&format!("/p2/repositories/{}/workspaces", repository_id))?,
        )
        .send_api()
    }

    /// Get Zenhub board for a repository.
    pub fn get_board(&self, repository_id: u64, workspace_id: &str) -> Result<Board, Error> {
        self.request(
            Method::GET,
            self.base_url.join(&format!(
                "/p2/workspaces/{}/repositories/{}/board",
                workspace_id, repository_id
            ))?,
        )
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
        )
        .send_api()
    }

    /// Set Zenhub StartDate for a milestone.
    pub fn set_start_date(
        &self,
        repository_id: u64,
        milestone_number: u32,
        start_date: &StartDate,
    ) -> Result<StartDate, Error> {
        self.request(
            Method::POST,
            self.base_url.join(&format!(
                "/p1/repositories/{}/milestones/{}/start_date",
                repository_id, milestone_number
            ))?,
        )
        .json(&start_date)
        .send_api()
    }

    /// Get Zenhub issue metadata.
    pub fn get_issue(&self, repository_id: u64, issue_number: u32) -> Result<IssueEstimate, Error> {
        self.request(
            Method::GET,
            self.base_url.join(&format!(
                "/p1/repositories/{}/issues/{}",
                repository_id, issue_number
            ))?,
        )
        .send_api()
    }

    /// Get Zenhub data for a list of issues.
    pub fn get_issues(
        &self,
        repository_id: u64,
        workspace_id: &str,
        issue_numbers: &[u32],
    ) -> Result<Vec<IssueData>, Error> {
        let mut issue_data_query = IssueDataQuery::default();
        issue_data_query.workspace_id = workspace_id;
        issue_data_query.estimates = true;
        issue_data_query.pipelines = true;
        issue_data_query.epics = true;

        self.request(
            Method::POST,
            self.base_url.join(&format!(
                "/v5/repositories/{}/issues/zenhub-data",
                repository_id
            ))?,
        )
        .query(&issue_data_query)
        .form(
            &issue_numbers
                .iter()
                .map(|issue_number| ("issue_numbers[]", issue_number))
                .collect::<Vec<(&str, &u32)>>(),
        )
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
        )
        .json(&SetEstimate::from(estimate))
        .send_api_no_response()
    }

    /// Move issue to a Zenhub pipeline.
    pub fn move_issue(
        &self,
        repository_id: u64,
        workspace_id: &str,
        issue_number: u32,
        position: &PipelinePosition,
    ) -> Result<(), Error> {
        self.request(
            Method::POST,
            self.base_url.join(&format!(
                "/p2/workspaces/{}/repositories/{}/issues/{}/moves",
                workspace_id, repository_id, issue_number
            ))?,
        )
        .json(position)
        .send_api_no_response()
    }
}

/// Zenhub Workspace.
#[derive(Deserialize, Serialize, Debug, Clone, Default, PartialEq)]
pub struct Workspace {
    pub name: Option<String>,
    pub description: Option<String>,
    pub id: String,
    pub repositories: Vec<u64>,
}

/// Zenhub issue estimate data, returned for a single issue.
#[derive(Deserialize, Serialize, Debug, Clone, Default, PartialEq)]
pub struct IssueEstimate {
    pub estimate: Option<Estimate>,
    pub is_epic: bool,
}

/// Zenhub issue data, returned for multiple issues.
#[derive(Deserialize, Serialize, Debug, Clone, Default, PartialEq)]
pub struct IssueData {
    pub issue_number: u32,
    pub estimate: Option<u32>,
    pub is_epic: bool,
    pub pipeline: IssueDataPipeline,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default, PartialEq)]
pub struct IssueDataPipeline {
    pub name: String,
}

/// Zenhub issue data, returned for multiple issues.
///
/// Bools are ints in this request. Hooray.
#[derive(Serialize, Debug, Clone, Default, PartialEq)]
pub struct IssueDataQuery<'a> {
    #[serde(rename = "workspaceId")]
    pub workspace_id: &'a str,
    #[serde(serialize_with = "serialize_bool_as_int")]
    #[serde(skip_serializing_if = "Not::not")]
    pub epics: bool,
    #[serde(serialize_with = "serialize_bool_as_int")]
    #[serde(skip_serializing_if = "Not::not")]
    pub estimates: bool,
    #[serde(serialize_with = "serialize_bool_as_int")]
    #[serde(skip_serializing_if = "Not::not")]
    pub pipelines: bool,
    #[serde(serialize_with = "serialize_bool_as_int")]
    #[serde(skip_serializing_if = "Not::not")]
    pub priorities: bool,
}

/// A Zenhub estimate.
#[derive(Deserialize, Serialize, Debug, Clone, Default, PartialEq)]
pub struct Estimate {
    pub value: u32,
}

impl From<&u32> for Estimate {
    fn from(value: &u32) -> Self {
        Estimate { value: *value }
    }
}

impl fmt::Display for Estimate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value.to_string())
    }
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

impl From<DateTime<FixedOffset>> for StartDate {
    fn from(datetime: DateTime<FixedOffset>) -> Self {
        Self {
            start_date: datetime,
        }
    }
}

fn serialize_bool_as_int<S>(value: &bool, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_i8(*value as i8)
}

#[cfg(test)]
pub mod tests {
    use lazy_static::lazy_static;
    use mockito::mock;
    use pretty_assertions::assert_eq;

    use super::*;

    const MOCK_ZENHUB_TOKEN: &str = "mock_token";
    lazy_static! {
        pub static ref MOCK_ZENHUB_CLIENT: Client =
            Client::new(&mockito::server_url(), MOCK_ZENHUB_TOKEN)
                .expect("Couldn't create mock zenhub client");
    }

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

    #[test]
    fn test_get_issue() {
        let body = r#"{
    "estimate": {
        "value": 3
    },
    "is_epic": false
}"#;

        let mock = mock("GET", "/p1/repositories/1234/issues/1")
            .match_header("x-authentication-token", "mock_token")
            .with_status(200)
            .with_body(body)
            .create();

        let issue = MOCK_ZENHUB_CLIENT.get_issue(1234, 1).unwrap();
        mock.assert();

        assert_eq!(
            issue,
            IssueEstimate {
                estimate: Some(Estimate { value: 3 }),
                is_epic: false,
            }
        );
    }

    #[test]
    fn test_get_issues() {
        let body = r#"[
  {
    "issue_number": 0,
    "estimate": 3,
    "is_epic": false,
    "pipeline": { "name": "Done" }
  },
  {
    "issue_number": 3,
    "estimate": null,
    "is_epic": true,
    "pipeline": { "name": "In Progress" }
  }
]"#;

        let mock = mock(
            "GET",
            "/v5/repositories/1234/issues/zenhub-data?workspaceId=workspace0&epics=1&estimates=1&pipelines=1",
        )
        .match_header("x-authentication-token", "mock_token")
        .match_header("content-type", "application/x-www-form-urlencoded")
        .match_body("issue_numbers%5B%5D=0&issue_numbers%5B%5D=2")
        .with_status(200)
        .with_body(body)
        .create();

        let issues = MOCK_ZENHUB_CLIENT
            .get_issues(1234, "workspace0", &[0, 2])
            .unwrap();
        mock.assert();

        assert_eq!(
            issues,
            vec![
                IssueData {
                    issue_number: 0,
                    estimate: Some(3),
                    is_epic: false,
                    pipeline: IssueDataPipeline {
                        name: "Done".to_owned()
                    }
                },
                IssueData {
                    issue_number: 3,
                    estimate: None,
                    is_epic: true,
                    pipeline: IssueDataPipeline {
                        name: "In Progress".to_owned()
                    }
                }
            ]
        );
    }
}
