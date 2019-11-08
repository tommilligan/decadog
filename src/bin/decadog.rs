use std::env;

use decadog::{AssignedTo, Client};
use dialoguer::{Confirmation, Input, Select};

fn main() {
    // Load token from env
    let github_token = env::var("GITHUB_TOKEN").expect("No GITHUB_TOKEN");

    let client = Client::new(&github_token);

    // Select milestone to move tickets to
    let milestones = client.get_milestones();
    if milestones.len() == 0 {
        eprintln!("No open milestones.");
        return ();
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
        .interact()
        .unwrap();
    let milestone = &milestones[selection];

    eprintln!("Loading organisation memebers...");
    let organisation_members = client.get_members();
    let organisation_member_names = organisation_members
        .iter()
        .map(|organisation_member| &organisation_member.login)
        .collect::<Vec<&String>>();

    loop {
        // Input an issue number
        let issue_number = Input::<String>::new()
            .with_prompt("Issue number")
            .interact()
            .expect("Failed interaction");

        // Fetch the issue
        let issue = client.get_issue_by_number(&issue_number);
        eprintln!("{}", issue);

        // If already assigned to the target milestone, no-op
        if issue.assigned_to(&milestone) {
            eprintln!("Already in milestone");
        } else {
            // Otherwise, confirm the assignment
            if Confirmation::new()
                .with_text("Assign milestone?")
                .interact()
                .expect("Failed interation")
            {
                client.assign_issue_to_milestone(&issue, &milestone);
            }
        }

        let selection = Select::new()
            .with_prompt("Assign")
            .default(0)
            .items(&organisation_member_names)
            .interact()
            .unwrap();
        let organisation_member = &organisation_members[selection];
        if !organisation_member.assigned_to(&issue) {
            client.assign_member_to_issue(&organisation_member, &issue);
        }
    }
}
