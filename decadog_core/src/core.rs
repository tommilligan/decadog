use crate::github::{Issue, Milestone, OrganisationMember};
use crate::zenhub::{Pipeline, StartDate};

/// Represents objects in the Github ontology that can be assigned to one another.
///
/// e.g. `User` assigned to `Issue`, `Issue` assigned to `Milestone`
pub trait AssignedTo<T> {
    fn assigned_to(&self, assignable: &T) -> bool;
}

/// A sprint.
#[derive(Debug, Clone)]
pub struct Sprint {
    pub milestone: Milestone,
    pub start_date: StartDate,
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
    use chrono::{DateTime, FixedOffset, NaiveDateTime};
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
                labels: Default::default(),
                created_at: *DEFAULT_DATETIME_FIXED,
                updated_at: *DEFAULT_DATETIME_FIXED,
                closed_at: Some(*DEFAULT_DATETIME_FIXED),
                html_url: Default::default(),
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
