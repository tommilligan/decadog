use std::collections::HashMap;
use std::env;

use decadog::{AssignedTo, Client, OrganisationMember};
use dialoguer::{Confirmation, Input, Select};
use scout;

fn main() -> Result<(), Error> {
    // Load token from env
    let github_token = env::var("GITHUB_TOKEN").expect("No GITHUB_TOKEN");

    let client = Client::new(&github_token)?;

    // Select milestone to move tickets to
    let milestones = client.get_milestones();
    if milestones.len() == 0 {
        eprintln!("No open milestones.");
        return Ok(());
    }

    let selection = Select::new()
        .with_prompt("Select milestone")
        .default(0)
        .items(
            &milestones
                .iter()
                .map(|milestone| &milestone.title)
                .collect::<Vec<&String>>(),
        )
        .interact()?;

    let milestone = &milestones[selection];

    eprintln!("Loading organisation memebers...");
    let organisation_members = client.get_members();
    let members_by_login: HashMap<String, OrganisationMember> = organisation_members
        .into_iter()
        .map(|member| (member.login.clone(), member))
        .collect();
    let member_logins: Vec<&str> = members_by_login.keys().map(|login| &login[..]).collect();

    loop {
        // Input an issue number
        let issue_number = Input::<String>::new()
            .with_prompt("Issue number")
            .interact()?;

        // Fetch the issue
        let issue = client.get_issue_by_number(&issue_number);
        eprintln!("{}", issue);

        // If already assigned to the target milestone, no-op
        if issue.assigned_to(&milestone) {
            eprintln!("Already in milestone.");
        } else {
            // Otherwise, confirm the assignment
            if Confirmation::new()
                .with_text("Assign milestone?")
                .interact()?
            {
                client.assign_issue_to_milestone(&issue, &milestone);
            }
        }

        let assignment_prompt = if issue.assignees.len() == 0 {
            "Assign member?".to_owned()
        } else {
            format!(
                "Currently assigned to {}. Update?",
                issue
                    .assignees
                    .iter()
                    .map(|member| member.login.clone())
                    .collect::<Vec<String>>()
                    .join(", ")
            )
        };
        if Confirmation::new()
            .with_text(&assignment_prompt)
            .interact()?
        {
            let member_login = scout::start(member_logins.clone(), vec![])?;
            let organisation_member = match members_by_login.get(&member_login) {
                Some(member_login) => member_login,
                None => continue,
            };
            if !organisation_member.assigned_to(&issue) {
                client.assign_member_to_issue(&organisation_member, &issue);
            }
        }
    }
}

mod error {
    use std::io::Error as IoError;

    use decadog::Error as DecadogError;
    use scout::errors::Error as ScoutError;
    use snafu::Snafu;

    #[derive(Debug, Snafu)]
    #[snafu(visibility = "pub")]
    pub enum Error {
        #[snafu(display("Decadog client error: {}", source))]
        Decadog { source: DecadogError },

        #[snafu(display("Scout error: {}", source))]
        Scout { source: ScoutError },

        #[snafu(display("Io error: {}", source))]
        Io { source: IoError },
    }

    impl From<DecadogError> for Error {
        fn from(source: DecadogError) -> Self {
            Error::Decadog { source }
        }
    }

    impl From<ScoutError> for Error {
        fn from(source: ScoutError) -> Self {
            Error::Scout { source }
        }
    }

    impl From<IoError> for Error {
        fn from(source: IoError) -> Self {
            Error::Io { source }
        }
    }
}
pub use error::Error;

pub type Result<T, E = Error> = std::result::Result<T, E>;
