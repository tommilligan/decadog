use std::collections::HashMap;

use clap::{App, ArgMatches, SubCommand};
use decadog::{AssignedTo, Client, OrganisationMember, Pipeline, PipelinePosition};
use dialoguer::{Confirmation, Input, Select};
use log::{debug, error, warn};
use scout;

use crate::{error::Error, Settings};

fn start_sprint(settings: &Settings) -> Result<(), Error> {
    let client = Client::new(
        &settings.owner,
        &settings.repo,
        &settings.github_token,
        settings.zenhub_token.as_ref().ok_or(Error::Settings {
            description: "Zenhub token required to start sprint.".to_owned(),
        })?,
    )?;

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

    debug!("Loading organisation members");
    let organisation_members = client.get_members()?;
    let members_by_login: HashMap<String, OrganisationMember> = organisation_members
        .into_iter()
        .map(|member| (member.login.clone(), member))
        .collect();
    let member_logins: Vec<&str> = members_by_login.keys().map(|login| &login[..]).collect();

    debug!("Loading repository");
    let repository = client.get_repository()?;
    debug!("Loading zenhub board");
    let board = client.get_board(&repository)?;
    let pipelines_by_name: HashMap<String, Pipeline> = board
        .pipelines
        .into_iter()
        .map(|pipeline| (pipeline.name.clone(), pipeline))
        .collect();
    let pipeline_names: Vec<&str> = pipelines_by_name
        .keys()
        .map(|pipeline_name| &pipeline_name[..])
        .collect();

    loop {
        if Confirmation::new().with_text("Next pipeline?").interact()? {
            let pipeline_name = scout::start(pipeline_names.clone(), vec![])?;
            let pipeline = match pipelines_by_name.get(&pipeline_name) {
                Some(pipeline_name) => pipeline_name,
                None => continue,
            };

            loop {
                // Input an issue number
                let issue_number = Input::<String>::new()
                    .with_prompt("Issue number (q to quit)")
                    .interact()?;

                // Fetch the issue
                if issue_number == "q" {
                    break;
                }

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
                    } else {
                        continue;
                    }
                }

                let position = PipelinePosition {
                    pipeline_id: pipeline.id.clone(),
                    position: "top".to_owned(),
                };
                debug!("Moving {} to {:?}", issue.number, position);
                client.move_issue(&repository, &issue, &position)?;

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
        } else {
            break;
        }
    }

    Ok(())
}

pub fn execute(matches: &ArgMatches, settings: &Settings) -> Result<(), Error> {
    if let (subcommand_name, Some(_)) = matches.subcommand() {
        match subcommand_name {
            "start" => {
                start_sprint(settings)?;
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
