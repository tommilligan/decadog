use chrono::{DateTime, Duration, FixedOffset, Local};
use colored::Colorize;
use decadog_core::github::{
    self, Milestone, OrganisationMember, Repository, SearchQueryBuilder, State,
};
use decadog_core::zenhub::{self, Estimate, Pipeline, Workspace};
use decadog_core::{AssignedTo, Client};
use lazy_static::lazy_static;
use log::error;
use structopt::StructOpt;

use crate::interact::{Confirm, FuzzySelect, Input, Select};
use crate::{error::Error, Settings};

lazy_static! {
    static ref ESTIMATES: Vec<Estimate> =
        [0u32, 1, 2, 3, 5, 8, 13].iter().map(Into::into).collect();
}

struct SprintPoints {
    pub planned: u32,
    pub in_milestone: u32,
    pub in_milestone_open: u32,

    pub done_in_sprint: u32,
    pub done_out_of_sprint: u32,
    pub done_total: u32,
}

impl SprintPoints {
    pub fn new(planned: u32, in_milestone: u32, in_milestone_open: u32) -> Result<Self, Error> {
        let done_in_sprint = planned
            .checked_sub(in_milestone_open)
            .ok_or_else(|| Error::User {
                description:
                    "Planned points too low: should be higher than points remaining in sprint."
                        .to_owned(),
            })?;
        let done_out_of_sprint = in_milestone
            .checked_sub(planned)
            .ok_or_else(|| Error::User {
                description:
                    "Planned points too high: should be lower than all points in milestone."
                        .to_owned(),
            })?;
        let done_total = done_in_sprint + done_out_of_sprint;

        Ok(Self {
            planned,
            in_milestone,
            in_milestone_open,

            done_in_sprint,
            done_out_of_sprint,
            done_total,
        })
    }
}

struct MilestoneManager<'a> {
    client: &'a Client<'a>,
    milestone: &'a Milestone,

    repository: Repository,
    workspace: Workspace,
    pipeline_options: FuzzySelect<Pipeline>,
    member_options: FuzzySelect<OrganisationMember>,
}

enum LoopStatus {
    Success,
    Quit,
    NextPipeline,
}

impl<'a> MilestoneManager<'a> {
    fn new(client: &'a Client<'a>, milestone: &'a Milestone) -> Result<Self, Error> {
        let organisation_members = client.get_members()?;
        let member_options: FuzzySelect<OrganisationMember> = organisation_members
            .into_iter()
            .map(|member| (member.login.clone(), member))
            .collect();

        let repository = client.get_repository()?;
        let workspace = client.get_first_workspace(&repository)?;

        let board = client.get_board(&repository, &workspace)?;
        let pipeline_options: FuzzySelect<Pipeline> = board
            .pipelines
            .into_iter()
            .map(|pipeline| (pipeline.name.clone(), pipeline))
            .collect();

        Ok(Self {
            client,
            milestone,
            repository,
            workspace,
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
        let issue_number_str = Input::<String>::new()
            .with_prompt("Issue number (n: next pipeline, q: quit)")
            .interact()?;

        // Fetch the issue and parse the number
        if issue_number_str == "q" {
            return Ok(LoopStatus::Quit);
        } else if issue_number_str == "n" {
            return Ok(LoopStatus::NextPipeline);
        }
        let issue_number = issue_number_str.parse().map_err(|_| Error::User {
            description: format!("Invalid issue number {}.", &issue_number_str),
        })?;

        let issue = self.client.get_issue(issue_number)?;
        eprintln!("{}", issue);

        // If already assigned to the target milestone, no-op
        if issue.assigned_to(self.milestone) {
            eprintln!("Already in milestone.");
        } else {
            // Otherwise, confirm the assignment
            if Confirm::new("Assign to milestone?").interact()? {
                self.client
                    .assign_issue_to_milestone(&issue, Some(&self.milestone))?;
            } else {
                return Ok(LoopStatus::Success);
            }
        }

        if issue.assigned_to(pipeline) {
            eprintln!("Already in pipeline.");
        } else {
            self.client.move_issue_to_pipeline(
                &self.repository,
                &self.workspace,
                &issue,
                &pipeline,
            )?;
        }

        let update_assignment = if issue.assignees.is_empty() {
            // If we do not have an assignee, default to updating assignment
            !Confirm::new("Leave unassigned?").interact()?
        } else {
            // If we already have assignee(s), default to existing value
            !Confirm::new(&format!(
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

fn sync_sprint(settings: &Settings) -> Result<(), Error> {
    let github = github::Client::new(&settings.github_url, &settings.github_token.value())?;
    let zenhub = zenhub::Client::new(
        settings
            .zenhub_url
            .as_ref()
            .ok_or(Error::Settings {
                description: "Zenhub url required to sync sprint.".to_owned(),
            })?
            .as_ref(),
        settings
            .zenhub_token
            .as_ref()
            .ok_or(Error::Settings {
                description: "Zenhub token required to sync sprint.".to_owned(),
            })?
            .as_ref(),
    )?;
    let client = Client::new(&settings.owner, &settings.repo, &github, &zenhub)?;

    // Select milestone to move tickets to
    let milestones = client.get_milestones()?;
    if milestones.is_empty() {
        eprintln!("No open milestones.");
        return Ok(());
    }

    let select_milestone =
        Select::new("Sprint to sync", &milestones).expect("At least one milestone is required.");
    let open_milestone = select_milestone.interact()?;

    let milestone_manager = MilestoneManager::new(&client, open_milestone)?;
    milestone_manager.manage()
}

fn create_sprint(settings: &Settings) -> Result<(), Error> {
    let github = github::Client::new(&settings.github_url, &settings.github_token.value())?;
    let zenhub = zenhub::Client::new(
        settings
            .zenhub_url
            .as_ref()
            .ok_or(Error::Settings {
                description: "Zenhub url required to create sprint.".to_owned(),
            })?
            .as_ref(),
        settings
            .zenhub_token
            .as_ref()
            .ok_or(Error::Settings {
                description: "Zenhub token required to create sprint.".to_owned(),
            })?
            .as_ref(),
    )?;
    let client = Client::new(&settings.owner, &settings.repo, &github, &zenhub)?;

    // Select milestone to move tickets to
    if Confirm::new("Create sprint from today for two weeks?").interact()? {
        let sprint_number = Input::<String>::new()
            .with_prompt("Sprint number")
            .interact()?;

        let repository = client.get_repository()?;
        // Zenhub UI uses dates with midday, so copy that here
        let start_date = DateTime::from_utc(
            Local::today().naive_local().and_hms(12, 00, 00),
            FixedOffset::east(0),
        );
        let due_on = start_date + Duration::days(13);
        let sprint = client.create_sprint(&repository, &sprint_number, start_date, due_on)?;

        eprintln!("Created '{}'", sprint.milestone.title);
    }
    Ok(())
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

    let github = github::Client::new(&settings.github_url, &settings.github_token.value())?;
    let zenhub = zenhub::Client::new(
        settings
            .zenhub_url
            .as_ref()
            .ok_or(Error::Settings {
                description: "Zenhub url required to finish sprint.".to_owned(),
            })?
            .as_ref(),
        settings
            .zenhub_token
            .as_ref()
            .ok_or(Error::Settings {
                description: "Zenhub token required to finish sprint.".to_owned(),
            })?
            .as_ref(),
    )?;
    let client = Client::new(&settings.owner, &settings.repo, &github, &zenhub)?;

    let select_estimate =
        Select::new("Estimate", ESTIMATES.iter()).expect("At least one estimate is required.");

    // Select milestone to close
    let milestones = client.get_milestones()?;
    if milestones.is_empty() {
        eprintln!("No open milestones.");
        return Ok(());
    }

    let select_milestone =
        Select::new("Sprint to finish", &milestones).expect("At least one milestone is required.");
    let open_milestone = select_milestone.interact()?.to_owned();

    let repository = client.get_repository()?;
    let sprint = client.get_sprint(&repository, open_milestone)?;

    println!();
    println!("{}", "Issues for review:".bold());
    let out_of_sprint_issues = client
        .search_issues(
            SearchQueryBuilder::new()
                .no_milestone()
                .closed_on_or_after(&sprint.start_date.start_date)
                .not_label("Z-obsolete"),
        )?
        .collect::<Result<Vec<_>, _>>()?;
    let milestone_issues = client
        .search_issues(
            SearchQueryBuilder::new()
                .milestone(&sprint.milestone.title)
                .state(&State::Closed)
                .not_label("Z-obsolete"),
        )?
        .collect::<Result<Vec<_>, _>>()?;

    for issue in out_of_sprint_issues.into_iter().chain(milestone_issues) {
        // If assigned to a different milestone, ignore
        if let Some(milestone) = &issue.milestone {
            if milestone.id != sprint.milestone.id {
                continue;
            };
        };

        let zenhub_issue = client.get_zenhub_issue(&repository, &issue)?;
        // If it's an epic, ignore
        if zenhub_issue.is_epic {
            continue;
        };

        // We only want to show the issue description once
        let mut description_shown = false;
        let mut show_description_once = || {
            if !description_shown {
                println!("{} -> {}", &issue, &issue.html_url);
                description_shown = true;
            }
        };

        // If no milestone, ask to assign to open milestone and if applicable mark as not planned
        // If answer is no, ignore
        if issue.milestone.is_none() {
            show_description_once();
            if Confirm::new("Assign to milestone?").interact()? {
                client.assign_issue_to_milestone(&issue, Some(&sprint.milestone))?;
            } else {
                continue;
            }
        };

        if zenhub_issue.estimate == None {
            show_description_once();
            let new_estimate = select_estimate.interact()?;
            client.set_estimate(&repository, &issue, new_estimate.value)?;
        };
    }

    println!();
    println!("{}", "Issues open in sprint:".bold());
    let open_milestone_issues = client
        .search_issues(
            SearchQueryBuilder::new()
                .state(&State::Open)
                .milestone(&sprint.milestone.title),
        )?
        .collect::<Result<Vec<_>, _>>()?;
    for issue in open_milestone_issues.iter() {
        println!("{}", issue);
    }

    println!();
    // Update title with number of planned and completed points this sprint
    // Prompt user for number of planned points in the sprint
    let planned_points_str = Input::<String>::new()
        .with_prompt("Points planned this sprint (q: quit)")
        .interact()?;
    if planned_points_str == "q" {
        return Ok(());
    }
    let planned_points: u32 = planned_points_str.parse().map_err(|_| Error::User {
        description: format!("Invalid number of planned points {}.", &planned_points_str),
    })?;

    println!("Calucating points summary...");
    let mut points_in_milestone: u32 = 0;
    let mut points_in_milestone_open: u32 = 0;
    let milestone_issues = client
        .search_issues(SearchQueryBuilder::new().milestone(&sprint.milestone.title))?
        .collect::<Result<Vec<_>, _>>()?;
    for issue in milestone_issues.into_iter() {
        let zenhub_issue = client.get_zenhub_issue(&repository, &issue)?;
        let issue_estimate = match zenhub_issue.estimate {
            Some(estimate) => estimate.value,
            None => 0,
        };
        if issue.state == State::Open {
            points_in_milestone_open += issue_estimate;
        };
        points_in_milestone += issue_estimate;
    }

    let sprint_points = SprintPoints::new(
        planned_points,
        points_in_milestone,
        points_in_milestone_open,
    )?;

    eprintln!(
        r#"*{}* Report
---
We completed *{}* planned points out of *{}* ({} remaining).
We also did {} out of sprint points.
In total, we finished *{} points* of work."#,
        sprint.milestone.title,
        sprint_points.done_in_sprint,
        sprint_points.planned,
        sprint_points.planned - sprint_points.done_in_sprint,
        sprint_points.done_out_of_sprint,
        sprint_points.done_total
    );
    eprintln!();

    if Confirm::new("Close sprint?").interact()? {
        // New title: Sprint <milestone_number> [<points done in sprint>/<points planned> + <points
        // done out of sprint>]
        let new_title = format!(
            "{} [{}/{} + {}]",
            sprint.milestone.title,
            sprint_points.done_in_sprint,
            sprint_points.planned,
            sprint_points.done_out_of_sprint
        );
        client.update_milestone_title(&sprint.milestone, new_title)?;

        println!("Closing milestone.");
        client.close_milestone(&sprint.milestone)?;
        println!("Removing open issues from milestone...");
        for issue in open_milestone_issues.iter() {
            client.assign_issue_to_milestone(&issue, None)?;
        }
    } else {
        return Ok(());
    }

    Ok(())
}

#[derive(Debug, StructOpt)]
pub enum Command {
    #[structopt(name = "create")]
    /// Create a new sprint.
    Create,

    #[structopt(name = "sync")]
    /// Sync a physical board to the digital board.
    Sync,

    #[structopt(name = "finish")]
    /// Finish an open sprint.
    Finish,
}

pub fn run(command: &Command, settings: &Settings) -> Result<(), Error> {
    match command {
        Command::Create => create_sprint(settings),
        Command::Sync => sync_sprint(settings),
        Command::Finish => finish_sprint(settings),
    }
}
