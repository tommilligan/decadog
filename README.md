# decadog

[![CircleCI branch](https://img.shields.io/circleci/project/github/tommilligan/decadog/master.svg)](https://circleci.com/gh/tommilligan/decadog)

Github toolkit. Octocat++.

## Installation

`git clone` followed by `cargo run`

## Useage

You will need a Github API token (probably a PAT token). You can generate this from
the settings page in the Github API. Currently, the scope of the project is `repo`.

Set this token in the environment as `GITHUB_TOKEN` before running.

## Todo

- [x] Make assigning multiple tickets to the same milestone painless
  - [x] Assign to milestone
  - [x] Assign to users
    - [ ] Fuzzy aided selection of users from cli
- [ ] CLI interface
  - [ ] make this one subcommand
  - [ ] parameterise owner/repo fields
  - [ ] config file for frequent use, from pwd
- [ ] Error handling
  - [ ] Verify response of assignment
