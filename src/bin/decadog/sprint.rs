use std::collections::HashMap;
use std::env;

use clap::{App, ArgMatches, SubCommand};
use decadog::{AssignedTo, Client, OrganisationMember};
use dialoguer::{Confirmation, Input, Select};
use log::{error, info};
use scout;

use crate::error::Error;

fn start_sprint() -> Result<(), Error> {
    // Load token from env
    let github_token = env::var("GITHUB_TOKEN").expect("No GITHUB_TOKEN");

    let client = Client::new(&github_token, "reinfer", "platform")?;

    // Select milestone to move tickets to
    let milestones = client.get_milestones()?;
    if milestones.len() == 0 {
        error!("No open milestones.");
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

    info!("Loading organisation memebers...");
    let organisation_members = client.get_members()?;
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
        let issue = client.get_issue_by_number(&issue_number)?;
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
                client.assign_issue_to_milestone(&issue, &milestone)?;
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
                client.assign_member_to_issue(&organisation_member, &issue)?;
            }
        }
    }
}

pub fn execute(matches: &ArgMatches) -> Result<(), Error> {
    if let (subcommand_name, Some(_)) = matches.subcommand() {
        match subcommand_name {
            "start" => {
                start_sprint()?;
            }
            _ => error!("Invalid subcommand."),
        }
    }
    Ok(())
}

pub fn subcommand<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name("sprint")
        .about("Manage sprints.")
        .subcommand(
            SubCommand::with_name("start")
                .about("Assign issues to a sprint, and people to issues."),
        )
}
