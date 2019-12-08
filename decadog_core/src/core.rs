use std::fmt;

use chrono::{DateTime, FixedOffset};
use serde_derive::{Deserialize, Serialize};

/// Represents objects in the Github ontology that can be assigned to one another.
///
/// e.g. `User` assigned to `Issue`, `Issue` assigned to `Milestone`
pub trait AssignedTo<T> {
    fn assigned_to(&self, assignable: &T) -> bool;
}

/// A Zenhub estimate.
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct Estimate {
    pub value: u32,
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
    pub position: u32,
}

/// Zenhub issue data.
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct ZenhubIssue {
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
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct PipelinePosition {
    pub pipeline_id: String,
    pub position: String,
}

/// A Zenhub board.
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct Board {
    pub pipelines: Vec<Pipeline>,
}

/// A Github Milestone.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Milestone {
    pub id: u32,
    pub number: u32,
    pub title: String,
    pub state: String,
    pub due_on: DateTime<FixedOffset>,
}

/// A Zenhub milestone StartDate.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct StartDate {
    pub start_date: DateTime<FixedOffset>,
}

/// A sprint.
#[derive(Debug, Clone)]
pub struct Sprint<'a> {
    pub milestone: &'a Milestone,
    pub start_date: StartDate,
}

/// A memeber reference in an Organisation.
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
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

/// A Github Issue.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Issue {
    pub id: u32,
    pub number: u32,
    pub state: String,
    pub title: String,
    pub milestone: Option<Milestone>,
    pub assignees: Vec<OrganisationMember>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
    pub closed_at: Option<DateTime<FixedOffset>>,
}

/// A Github Repository.
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct Repository {
    pub id: u32,
    pub name: String,
}

impl fmt::Display for Milestone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.title, self.state)
    }
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.number, self.title)
    }
}

impl AssignedTo<Milestone> for Issue {
    fn assigned_to(&self, assignable: &Milestone) -> bool {
        if let Some(issue_milestone) = &self.milestone {
            if issue_milestone.id == assignable.id {
                return true;
            }
        }
        false
    }
}

impl AssignedTo<Pipeline> for Issue {
    fn assigned_to(&self, assignable: &Pipeline) -> bool {
        assignable
            .issues
            .iter()
            .any(|issue| issue.issue_number == self.number)
    }
}

impl AssignedTo<Issue> for OrganisationMember {
    fn assigned_to(&self, assignable: &Issue) -> bool {
        assignable
            .assignees
            .iter()
            .any(|organisation_member| organisation_member.login == self.login)
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;
    use lazy_static::lazy_static;

    use super::*;

    lazy_static! {
        static ref DEFAULT_DATETIME_FIXED: DateTime<FixedOffset> =
            { DateTime::from_utc(NaiveDateTime::from_timestamp(0, 0), FixedOffset::east(0)) };
    }

    impl Default for Issue {
        fn default() -> Self {
            Issue {
                id: Default::default(),
                number: Default::default(),
                state: Default::default(),
                title: Default::default(),
                milestone: Default::default(),
                assignees: Default::default(),
                created_at: *DEFAULT_DATETIME_FIXED,
                updated_at: *DEFAULT_DATETIME_FIXED,
                closed_at: Some(*DEFAULT_DATETIME_FIXED),
            }
        }
    }

    impl Default for Milestone {
        fn default() -> Self {
            Milestone {
                id: Default::default(),
                number: Default::default(),
                title: Default::default(),
                state: Default::default(),
                due_on: *DEFAULT_DATETIME_FIXED,
            }
        }
    }

    #[test]
    fn issue_assigned_to_milestone() {
        let milestone = Milestone::default();
        let issue = Issue::default();
        let mut issue_with_milestone = issue.clone();
        issue_with_milestone.milestone = Some(milestone.clone());
        assert!(!issue.assigned_to(&milestone));
        assert!(issue_with_milestone.assigned_to(&milestone));
    }

    #[test]
    fn member_assigned_to_issue() {
        let issue = Issue::default();
        let member = OrganisationMember::default();
        let mut issue_with_assignee = issue.clone();
        issue_with_assignee.assignees = vec![member.clone()];
        assert!(!member.assigned_to(&issue));
        assert!(member.assigned_to(&issue_with_assignee));
    }
}
