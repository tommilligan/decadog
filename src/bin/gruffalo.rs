use std::env;
use std::fmt;

use dialoguer::{Confirmation, Input};
use github_rs::client::{Executor, Github};
use github_rs::StatusCode;
use reqwest::header::{HeaderMap, AUTHORIZATION};
use serde::de::DeserializeOwned;
use serde_derive::{Deserialize, Serialize};

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

#[derive(Deserialize, Serialize, Debug, Clone)]
struct Milestone {
    id: u32,
    number: u32,
    title: String,
    state: String,
}

impl fmt::Display for Milestone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.title, self.state)
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct Issue {
    id: u32,
    number: u32,
    state: String,
    title: String,
    milestone: Option<Milestone>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct UpdateMilestone {
    milestone: u32,
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(milestone) = &self.milestone {
            write!(
                f,
                "{} ({}) [{}]: {}",
                self.number, self.state, milestone.title, self.title
            )
        } else {
            write!(f, "{} ({}): {}", self.number, self.state, self.title)
        }
    }
}

fn main() -> Result<(), reqwest::Error> {
    // Load token from env
    let github_token = env::var("GITHUB_TOKEN").expect("No GITHUB_TOKEN");

    // Setup clients
    // Nice github API
    let github_client = Github::new(&github_token).expect("Failed to create Github client");
    // Raw REST
    let reqwest_client = reqwest::Client::new();
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        format!("token {}", &github_token)
            .parse()
            .expect("Invalid auth header"),
    );

    // Select milestone to move tickets to
    let milestone_title = Input::<String>::new()
        .with_prompt("Milestone title:")
        .interact()
        .expect("Failed interaction");
    let milestones: Vec<Milestone> = reqwest_client
        .get("https://api.github.com/repos/reinfer/platform/milestones")
        .headers(headers.clone())
        .send()?
        .json()?;
    let milestone = milestones
        .into_iter()
        .find(|milestone| milestone.title == milestone_title)
        .expect("Could not find matching milestone");
    println!("{}", milestone);

    loop {
        let issue_number = Input::<String>::new()
            .with_prompt("Issue number")
            .interact()
            .expect("Failed interaction");
        let issue = github_client
            .get()
            .repos()
            .owner("reinfer")
            .repo("platform")
            .issues()
            .number(&issue_number.to_string())
            .try_execute::<Issue>()
            .expect("Failed to get issue");

        if let Some(issue_milestone) = &issue.milestone {
            if issue_milestone.id == milestone.id {
                println!("Already assigned to milestone");
                continue;
            }
        }

        println!("{}", issue);
        if Confirmation::new()
            .with_text("Assign milestone?")
            .interact()
            .expect("Failed interation")
        {
            println!("Assigning");
            reqwest_client
                .patch(&format!(
                    "https://api.github.com/repos/reinfer/platform/issues/{}",
                    issue.number
                ))
                .json(&UpdateMilestone {
                    milestone: milestone.number,
                })
                .headers(headers.clone())
                .send()?;
        } else {
            println!("Cancelled");
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_something() {}
}
