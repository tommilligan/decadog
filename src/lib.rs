use github_rs::client::{Executor, Github};
use github_rs::StatusCode;
use reqwest::header::{HeaderMap, AUTHORIZATION};
use reqwest::Client as ReqwestClient;
use serde::de::DeserializeOwned;
use serde_derive::Deserialize;

mod core;

pub use crate::core::{AssignedTo, Issue, IssuePatch, Milestone};

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

impl Client {
    pub fn new(token: &str) -> Self {
        // Nice github API
        let github_client = Github::new(token).expect("Failed to create Github client");

        // Raw REST endpoints
        let reqwest_client = reqwest::Client::new();
        let mut reqwest_headers = HeaderMap::new();
        reqwest_headers.insert(
            AUTHORIZATION,
            format!("token {}", token)
                .parse()
                .expect("Invalid auth header"),
        );

        Client {
            github_client,
            reqwest_client,
            reqwest_headers,
        }
    }

    pub fn get_milestone_by_title(&self, title: &str) -> Milestone {
        let milestones: Vec<Milestone> = self
            .reqwest_client
            .get("https://api.github.com/repos/reinfer/platform/milestones")
            .headers(self.reqwest_headers.clone())
            .send()
            .unwrap()
            .json()
            .unwrap();
        let milestone = milestones
            .into_iter()
            .find(|milestone| milestone.title == title)
            .expect("Could not find matching milestone");
        milestone
    }

    pub fn assign_issue_to_milestone(&self, issue: &Issue, milestone: &Milestone) -> () {
        self.reqwest_client
            .patch(&format!(
                "https://api.github.com/repos/reinfer/platform/issues/{}",
                issue.number
            ))
            .json(&IssuePatch {
                milestone: milestone.number,
            })
            .headers(self.reqwest_headers.clone())
            .send()
            .unwrap();
    }

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
}
