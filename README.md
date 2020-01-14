# decadog

[![CircleCI branch](https://img.shields.io/circleci/project/github/tommilligan/decadog/master.svg)](https://circleci.com/gh/tommilligan/decadog)

Github toolkit. Octocat++.

## Installation

Install using `cargo`:

```bash
cargo install --git https://github.com/tommilligan/decadog
```

### Configuration

`decadog` can be configured in several ways:

#### `decadog.yml`

A `decadog.yml` file in the current working directory. The specification is:

```yaml
version: 1

owner: Github username/organisation name
repo: Github repository

github_token: Github PAT token
zenhub_token: Zenhub API token (optional)
```

#### Environment variables

Any setting from the config file above can be set by a variable in all caps,
prefixed with `DECADOG_`, such as:

```bash
export DECADOG_GITHUB_TOKEN=abcdef...
```

#### OS Keyring (secrets only)

You will need to compile with `config_keyring` for this to work. You may need to
install `libdbus-1-dev`.

```bash
cargo build --release --bins --features config_keyring
```

On Linux, you can set the appropriate secrets by running:

First, install `secret-tool`:

```bash
apt-get install libsecret-tools
```

```bash
secret-tool store --label='decadog_github_token' application rust-keyring service decadog_github_token username decadog
# interactive password prompt...

secret-tool store --label='decadog_zenhub_token' application rust-keyring service decadog_zenhub_token username decadog
# interactive password prompt...
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
  - [ ] Make errors shown to users nicer
    - [ ] Make config errors clearer
  - [ ] Manage exit codes
