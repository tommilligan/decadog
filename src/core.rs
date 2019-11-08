use std::fmt;

use serde_derive::{Deserialize, Serialize};

/// Represents objects in the Github ontology that can be assigned to one another.
///
/// e.g. `User` assigned to `Issue`, `Issue` assigned to `Milestone`
pub trait AssignedTo<T> {
    fn assigned_to(&self, assignable: &T) -> bool;
}

/// A Github Milestone.
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct Milestone {
    pub id: u32,
    pub number: u32,
    pub title: String,
    pub state: String,
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
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct Issue {
    pub id: u32,
    pub number: u32,
    pub state: String,
    pub title: String,
    pub milestone: Option<Milestone>,
    pub assignees: Vec<OrganisationMember>,
}

impl fmt::Display for Milestone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.title, self.state)
    }
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
    use super::*;

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
