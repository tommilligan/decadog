#![deny(clippy::all)]

use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::Hasher;

use chrono::{DateTime, FixedOffset};

mod core;
pub mod error;
pub mod github;
pub mod secret;
pub mod zenhub;

pub use crate::core::{AssignedTo, Sprint};
pub use error::Error;
use github::{
    Direction, Issue, IssueUpdate, Milestone, OrganisationMember, Repository, SearchIssues,
};
use zenhub::{Board, Pipeline, PipelinePosition, StartDate};

/// Decadog client, used to abstract complex tasks over several APIs.
pub struct Client<'a> {
    owner: &'a str,
    repo: &'a str,
    github: &'a github::Client,
    zenhub: &'a zenhub::Client,

    id: u64,
}

impl<'a> fmt::Debug for Client<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Decadog client {}", self.id)
    }
}

impl<'a> Client<'a> {
    /// Create a new client that can make requests to the Github API using token auth.
    pub fn new(
        owner: &'a str,
        repo: &'a str,
        github: &'a github::Client,
        zenhub: &'a zenhub::Client,
    ) -> Result<Client<'a>, Error> {
        let mut hasher = DefaultHasher::new();
        hasher.write(owner.as_bytes());
        hasher.write(repo.as_bytes());
        hasher.write(&github.id().to_be_bytes());
        hasher.write(&zenhub.id().to_be_bytes());
        let id = hasher.finish();

        Ok(Client {
            id,
            owner,
            repo,
            github,
            zenhub,
        })
    }

    pub fn owner(&self) -> &str {
        self.owner
    }

    pub fn repo(&self) -> &str {
        self.repo
    }

    /// Get Zenhub StartDate for a Github Milestone.
    pub fn get_start_date(
        &self,
        repository: &Repository,
        milestone: &Milestone,
    ) -> Result<StartDate, Error> {
        self.zenhub.get_start_date(repository.id, milestone.number)
    }

    /// Get Zenhub board for a repository.
    pub fn get_board(&self, repository: &Repository) -> Result<Board, Error> {
        self.zenhub.get_board(repository.id)
    }

    /// Get Zenhub issue metadata.
    pub fn get_zenhub_issue(
        &self,
        repository: &Repository,
        issue: &Issue,
    ) -> Result<zenhub::Issue, Error> {
        self.zenhub.get_issue(repository.id, issue.number)
    }

    /// Set Zenhub issue estimate.
    pub fn set_estimate(
        &self,
        repository: &Repository,
        issue: &Issue,
        estimate: u32,
    ) -> Result<(), Error> {
        self.zenhub
            .set_estimate(repository.id, issue.number, estimate)
    }

    /// Get sprint for milestone.
    pub fn get_sprint<'b>(
        &self,
        repository: &Repository,
        milestone: &'b Milestone,
    ) -> Result<Sprint<'b>, Error> {
        let start_date = self.get_start_date(repository, milestone)?;
        Ok(Sprint {
            milestone,
            start_date,
        })
    }

    /// Move issue to a Zenhub pipeline.
    pub fn move_issue_to_pipeline(
        &self,
        repository: &Repository,
        issue: &Issue,
        pipeline: &Pipeline,
    ) -> Result<(), Error> {
        let mut position = PipelinePosition::default();
        position.pipeline_id = pipeline.id.clone();

        self.zenhub
            .move_issue(repository.id, issue.number, &position)
    }

    /// Get a repository from the API.
    pub fn get_repository(&self) -> Result<Repository, Error> {
        self.github.get_repository(self.owner, self.repo)
    }

    /// Get an issue from the API.
    pub fn get_issue(&self, issue_number: u32) -> Result<Issue, Error> {
        self.github.get_issue(self.owner, self.repo, issue_number)
    }

    /// Get milestones from the API.
    pub fn get_milestones(&self) -> Result<Vec<Milestone>, Error> {
        self.github.get_milestones(self.owner, self.repo)
    }

    /// Assign an issue to a milestone. Passing `None` will set to no milestone.
    ///
    /// This will overwrite an existing milestone, if present.
    pub fn assign_issue_to_milestone(
        &self,
        issue: &Issue,
        milestone: Option<&Milestone>,
    ) -> Result<Issue, Error> {
        let mut update = IssueUpdate::default();
        update.milestone = Some(milestone.map(|milestone| milestone.number));

        self.github
            .patch_issue(&self.owner, &self.repo, issue.number, &update)
    }

    /// Assign an organisation member to an issue.
    ///
    /// This will overwrite any existing assignees, if present.
    pub fn assign_member_to_issue(
        &self,
        member: &OrganisationMember,
        issue: &Issue,
    ) -> Result<Issue, Error> {
        let mut update = IssueUpdate::default();
        update.assignees = Some(vec![member.login.clone()]);

        self.github
            .patch_issue(&self.owner, &self.repo, issue.number, &update)
    }

    /// Get issues closed after the given datetime.
    pub fn get_issues_closed_after(
        &self,
        datetime: &DateTime<FixedOffset>,
    ) -> Result<Vec<Issue>, Error> {
        let query = SearchIssues {
            q: format!(
                "repo:{}/{} type:issue state:closed closed:>={}",
                self.owner,
                self.repo,
                datetime.format("%Y-%m-%d")
            ),
            sort: Some("updated".to_owned()),
            order: Some(Direction::Ascending),
        };
        self.github.search_issues(&query)
    }

    /// Get issues open in a given milestone.
    pub fn get_milestone_open_issues(&self, milestone: &Milestone) -> Result<Vec<Issue>, Error> {
        let query = SearchIssues {
            q: format!(
                r#"repo:{}/{} type:issue state:open milestone:"{}""#,
                self.owner, self.repo, milestone.title
            ),
            sort: Some("updated".to_owned()),
            order: Some(Direction::Ascending),
        };
        self.github.search_issues(&query)
    }

    /// Get organisation members.
    pub fn get_members(&self) -> Result<Vec<OrganisationMember>, Error> {
        self.github.get_members(self.owner)
    }
}
