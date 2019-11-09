# decadog

[![CircleCI branch](https://img.shields.io/circleci/project/github/tommilligan/decadog/master.svg)](https://circleci.com/gh/tommilligan/decadog)

Github toolkit. Octocat++.

## Installation

Install using `cargo`:

```bash
cargo install --git https://github.com/tommilligan/decadog
```

## Use

You will need a Github API token. You can generate this from the [Settings > Tokens](https://github.com/settings/tokens) page in the Github UI.
You should grant the scope `repo`.

Set this token in your environment as `DECADOG_GITHUB_TOKEN`.

See the [example configuration file](./tree/master/example/decadog.yml). This file should be in your current working directory.

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
- [x] CLI interface
  - [x] make this one subcommand
  - [x] parameterise owner/repo fields
  - [x] config file for frequent use, from pwd
- [x] Error handling
  - [x] Verify response of assignment
