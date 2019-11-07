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

/// A Github Issue.
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct Issue {
    pub id: u32,
    pub number: u32,
    pub state: String,
    pub title: String,
    pub milestone: Option<Milestone>,
}

/// Updates to an Issue, as expected in the `PATCH` update.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct IssuePatch {
    pub milestone: u32,
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
}
