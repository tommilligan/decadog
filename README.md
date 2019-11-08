# decadog

[![CircleCI branch](https://img.shields.io/circleci/project/github/tommilligan/decadog/master.svg)](https://circleci.com/gh/tommilligan/decadog)

Github toolkit. Octocat++.

## Installation

`git clone` followed by `cargo run`

## Usage

You will need a Github API token (probably a PAT token). You can generate this from
the settings page in the Github API. Currently, the scope of the project is `repo`.

Set this token in the environment as `GITHUB_TOKEN` before running.

### Start Sprint

The currently functionality aims to make starting a sprint easy. It assumes:

- you have already created an appropriate milestone for the sprint
- you have already created issues/users in your repo

It will:

- ask which milestone you want to populate
- prompt for ticket numbers, and for each:
  - display a description
  - confirm assigning it to the milestone
  - prompt to assign a user to the ticket

## Todo

- [x] Make assigning multiple tickets to the same milestone painless
  - [x] Assign to milestone
  - [x] Assign to users
    - [x] Fuzzy aided selection of users from cli
- [ ] CLI interface
  - [ ] make this one subcommand
  - [ ] parameterise owner/repo fields
  - [ ] config file for frequent use, from pwd
- [\] Error handling
  - [ ] Verify response of assignment
