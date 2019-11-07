use std::env;

use decadog::{AssignedTo, Client};
use dialoguer::{Confirmation, Input};

fn main() -> Result<(), reqwest::Error> {
    // Load token from env
    let github_token = env::var("GITHUB_TOKEN").expect("No GITHUB_TOKEN");

    let client = Client::new(&github_token);

    // Select milestone to move tickets to
    let milestone_title = Input::<String>::new()
        .with_prompt("Milestone title:")
        .interact()
        .expect("Failed interaction");
    let milestone = client.get_milestone_by_title(&milestone_title);
    println!("{}", milestone);

    loop {
        // Input an issue number
        let issue_number = Input::<String>::new()
            .with_prompt("Issue number")
            .interact()
            .expect("Failed interaction");

        // Fetch the issue
        let issue = client.get_issue_by_number(&issue_number);

        // If already assigned to the target milestone, no-op
        if issue.assigned_to(&milestone) {
            println!("Already assigned to milestone");
            continue;
        }

        // Otherwise, confirm the assignment
        println!("{}", issue);
        if Confirmation::new()
            .with_text("Assign milestone?")
            .interact()
            .expect("Failed interation")
        {
            println!("Assigning");
            client.assign_issue_to_milestone(&issue, &milestone);
        } else {
            println!("Cancelled");
        }
    }
}
