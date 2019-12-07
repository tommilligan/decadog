use clap::{App, ArgMatches, SubCommand};
use colored::Colorize;
use decadog_core::{
    AssignedTo, Client, Milestone, OrganisationMember, Pipeline, PipelinePosition, Repository,
};
use dialoguer::{Confirmation, Input, Select};
use indexmap::IndexMap;
use log::error;
use scout;

use crate::{error::Error, Settings};

/// A read-only `HashMap`, keyed by human readable description.
struct ScoutOptions<V> {
    lookup: IndexMap<String, V>,
}

impl<V> ScoutOptions<V> {
    pub fn new(lookup: IndexMap<String, V>) -> Self {
        Self { lookup }
    }

    fn get(&self, key: &str) -> Option<&V> {
        self.lookup.get(key)
    }

    fn keys(&self) -> Vec<&str> {
        self.lookup.keys().map(|key| &**key).collect()
    }

    fn interact(&self) -> Result<&V, Error> {
        let chosen_key = scout::start(self.keys(), vec![])?;
        self.get(&chosen_key).ok_or(Error::User {
            description: format!("Unknown pipeline choice '{}'", chosen_key),
        })
    }
}

struct MilestoneManager<'a> {
    client: &'a Client<'a>,
    milestone: &'a Milestone,

    repository: Repository,
    pipeline_options: ScoutOptions<Pipeline>,
    member_options: ScoutOptions<OrganisationMember>,
}

enum LoopStatus {
    Success,
    Quit,
    NextPipeline,
}

impl<'a> MilestoneManager<'a> {
    fn new(client: &'a Client<'a>, milestone: &'a Milestone) -> Result<Self, Error> {
        let organisation_members = client.get_members()?;
        let members_by_login = organisation_members
            .into_iter()
            .map(|member| (member.login.clone(), member))
            .collect();
        let member_options = ScoutOptions::new(members_by_login);

        let repository = client.get_repository()?;

        let board = client.get_board(&repository)?;
        let pipelines_by_name = board
            .pipelines
            .into_iter()
            .map(|pipeline| (pipeline.name.clone(), pipeline))
            .collect();
        let pipeline_options = ScoutOptions::new(pipelines_by_name);

        Ok(Self {
            client,
            milestone,
            repository,
            member_options,
            pipeline_options,
        })
    }

    fn manage(&self) -> Result<(), Error> {
        loop {
            let pipeline = self.pipeline_options.interact()?;
            loop {
                match self.manage_issue(pipeline) {
                    Ok(LoopStatus::Success) => continue,
                    Ok(LoopStatus::NextPipeline) => break,
                    Ok(LoopStatus::Quit) => return Ok(()),
                    Err(error) => error!("{}", error),
                }
            }
        }
    }

    fn manage_issue(&self, pipeline: &Pipeline) -> Result<LoopStatus, Error> {
        // Input an issue number
        let issue_number = Input::<String>::new()
            .with_prompt("Issue number (n: next pipeline, q: quit)")
            .interact()?;

        // Fetch the issue
        if issue_number == "q" {
            return Ok(LoopStatus::Quit);
        } else if issue_number == "n" {
            return Ok(LoopStatus::NextPipeline);
        }

        let issue = self.client.get_issue_by_number(&issue_number)?;
        eprintln!("{}", issue);

        // If already assigned to the target milestone, no-op
        if issue.assigned_to(self.milestone) {
            eprintln!("Already in milestone.");
        } else {
            // Otherwise, confirm the assignment
            if Confirmation::new()
                .with_text("Assign to milestone?")
                .interact()?
            {
                self.client
                    .assign_issue_to_milestone(&issue, &self.milestone)?;
            } else {
                return Ok(LoopStatus::Success);
            }
        }

        let position = PipelinePosition {
            pipeline_id: pipeline.id.clone(),
            position: "top".to_owned(),
        };

        if issue.assigned_to(pipeline) {
            eprintln!("Already in pipeline.");
        } else {
            self.client
                .move_issue(&self.repository, &issue, &position)?;
        }

        let update_assignment = if issue.assignees.is_empty() {
            // If we do not have an assignee, default to updating assignment
            !Confirmation::new()
                .with_text("Leave unassigned?")
                .interact()?
        } else {
            // If we already have assignee(s), default to existing value
            !Confirmation::new()
                .with_text(&format!(
                    "Assigned to {}; is this correct?",
                    issue
                        .assignees
                        .iter()
                        .map(|member| member.login.clone())
                        .collect::<Vec<String>>()
                        .join(", ")
                ))
                .interact()?
        };

        if update_assignment {
            let organisation_member = self.member_options.interact()?;
            if !organisation_member.assigned_to(&issue) {
                self.client
                    .assign_member_to_issue(&organisation_member, &issue)?;
            };
        }

        Ok(LoopStatus::Success)
    }
}

fn start_sprint(settings: &Settings) -> Result<(), Error> {
    let client = Client::new(
        &settings.owner,
        &settings.repo,
        settings.github_token.as_ref(),
        settings
            .zenhub_token
            .as_ref()
            .ok_or(Error::Settings {
                description: "Zenhub token required to start sprint.".to_owned(),
            })?
            .as_ref(),
    )?;

    // Select milestone to move tickets to
    let milestones = client.get_milestones()?;
    if milestones.is_empty() {
        eprintln!("No open milestones.");
        return Ok(());
    }

    let selection = Select::new()
        .with_prompt("Sprint to start")
        .default(0)
        .items(
            &milestones
                .iter()
                .map(|milestone| &milestone.title)
                .collect::<Vec<&String>>(),
        )
        .interact()?;

    let milestone = &milestones[selection];
    let milestone_manager = MilestoneManager::new(&client, milestone)?;
    milestone_manager.manage()
}

fn finish_sprint(settings: &Settings) -> Result<(), Error> {
    // To count as points in the sprint, the ticket must have been
    // - closed in the sprint period
    // - have points assigned
    //
    // For each ticket *closed* in the sprint time range (start to *now*)
    // - if it has no milestone attached, prompt to attach to open milestone
    // - if it has no points, prompt to assign estimate
    //
    // For each non-closed ticket in the sprint
    // - print status, ask if correct

    let client = Client::new(
        &settings.owner,
        &settings.repo,
        settings.github_token.as_ref(),
        settings
            .zenhub_token
            .as_ref()
            .ok_or(Error::Settings {
                description: "Zenhub token required to finish sprint.".to_owned(),
            })?
            .as_ref(),
    )?;

    let estimates: [u32; 7] = [0, 1, 2, 3, 5, 8, 13];

    // Select milestone to close
    let milestones = client.get_milestones()?;
    if milestones.is_empty() {
        eprintln!("No open milestones.");
        return Ok(());
    }

    let selection = Select::new()
        .with_prompt("Sprint to finish")
        .default(0)
        .items(
            &milestones
                .iter()
                .map(|milestone| &milestone.title)
                .collect::<Vec<&String>>(),
        )
        .interact()?;

    let open_milestone = &milestones[selection];
    let repository = client.get_repository()?;
    let sprint = client.get_sprint(&repository, &open_milestone)?;

    println!();
    println!("{}", "Issues closed in the sprint timeframe:".bold());
    for issue in client
        .get_issues_closed_after(&sprint.start_date.start_date)?
        .iter()
    {
        // If assigned to a different milestone, ignore
        if let Some(milestone) = &issue.milestone {
            if milestone.id != open_milestone.id {
                continue;
            };
        };

        println!("{}", &issue);

        // If no milestone, ask to assign to open milestone
        // If answer is no, ignore this issue
        if issue.milestone.is_none() {
            if Confirmation::new()
                .with_text("Assign to milestone?")
                .interact()?
            {
                client.assign_issue_to_milestone(&issue, &open_milestone)?;
            } else {
                continue;
            }
        };

        let zenhub_issue = client.get_zenhub_issue(&repository, &issue)?;
        if zenhub_issue.estimate.is_none() {
            let selection = Select::new()
                .with_prompt("Estimate")
                .default(0)
                .items(
                    &estimates
                        .iter()
                        .map(|estimate| estimate.to_string())
                        .collect::<Vec<String>>(),
                )
                .interact()?;

            let estimate = estimates[selection];
            client.set_estimate(&repository, &issue, estimate)?;
        }
    }

    println!();
    println!("{}", "Issues open in sprint:".bold());
    for issue in client.get_milestone_open_issues(&open_milestone)?.iter() {
        println!("{}", issue);
    }

    if Confirmation::new().with_text("Close sprint?").interact()? {
        error!("Closing sprint not implemented.")
    } else {
        return Ok(());
    }

    Ok(())
}

pub fn execute(matches: &ArgMatches, settings: &Settings) -> Result<(), Error> {
    if let (subcommand_name, Some(_)) = matches.subcommand() {
        match subcommand_name {
            "start" => {
                start_sprint(settings)?;
            }
            "finish" => {
                finish_sprint(settings)?;
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
        .subcommand(SubCommand::with_name("finish").about("Tidy up and close a sprint."))
}
