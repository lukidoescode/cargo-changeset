# cargo-changeset

[![Crates.io](https://img.shields.io/crates/v/cargo-changeset)](https://crates.io/crates/cargo-changeset)
[![docs.rs](https://img.shields.io/docsrs/cargo-changeset)](https://docs.rs/cargo-changeset)
[![License: MIT](https://img.shields.io/crates/l/cargo-changeset)](https://github.com/lukidoescode/cargo-changeset/blob/main/LICENSE)
[![CI](https://img.shields.io/github/actions/workflow/status/lukidoescode/cargo-changeset/ci.yml?branch=main&label=CI)](https://github.com/lukidoescode/cargo-changeset/actions/workflows/ci.yml)
[![Security Audit](https://img.shields.io/github/actions/workflow/status/lukidoescode/cargo-changeset/audit.yml?branch=main&label=audit)](https://github.com/lukidoescode/cargo-changeset/actions/workflows/audit.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://gist.githubusercontent.com/lukidoescode/8b0cd92399fa481ee4649cd448413316/raw/cargo-changeset-coverage.json)](https://github.com/lukidoescode/cargo-changeset/actions/workflows/coverage.yml)
[![MSRV](https://img.shields.io/badge/MSRV-1.85.0-blue)](https://github.com/lukidoescode/cargo-changeset/blob/main/Cargo.toml)
[![Downloads](https://img.shields.io/crates/d/cargo-changeset)](https://crates.io/crates/cargo-changeset)

> [!NOTE]
> `cargo-changeset` is under active development and is used to manage its own releases. The core workflow is stable, but some features may still evolve. Community testing is very welcome — if you run into any issues, please [open an issue](https://github.com/lukidoescode/cargo-changeset/issues).

## What is cargo-changeset?

Releasing Cargo packages — especially in a workspace with multiple crates — requires coordinating version bumps, writing changelog entries, updating dependency declarations, committing, and tagging. Doing this by hand is tedious and error-prone.

`cargo-changeset` solves this by introducing **changeset files**: small Markdown files that declare which crates are affected, what kind of version bump each needs, and a human-readable summary of the change. Contributors create these files alongside their code changes. At release time, the tool aggregates all pending changesets and automatically bumps versions in `Cargo.toml`, updates internal dependency versions, generates [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) entries, and creates git commits and tags.

It works with single-crate projects and multi-crate workspaces alike, supports pre-release versions (alpha, beta, rc), and is fully usable in CI environments without interactive prompts.

## How It Works

The release cycle has three phases:

1. **Capture intent** — When making changes, contributors create a changeset file declaring which crates are affected, what bump type each needs, and a summary of the change.
2. **Verify coverage** — In CI, `cargo changeset verify` ensures every modified crate has at least one changeset before a pull request can merge.
3. **Release** — `cargo changeset release` consumes all pending changesets, computes new versions, updates `Cargo.toml` files, generates changelog entries, and creates a git commit with tags.

A changeset file is a Markdown file with YAML front matter, stored in `.changeset/changesets/`:

```markdown
---
category: added
changeset-project: minor
---

Add config option to customize the changelog message template for dependency updates
```

Each file lists one or more crates with a bump type (`major`, `minor`, `patch`, or `none`) and an optional [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) category (`added`, `changed`, `deprecated`, `removed`, `fixed`, `security`). The body is a free-form description that becomes the changelog entry. The `none` type allows documenting changes without incrementing the version — by default these are promoted to `patch` bumps on release, but this behavior is [configurable](#configuration).

### Features

| Feature | Details |
|---------|---------|
| **Workspace & single-crate support** | Virtual workspaces, workspaces with a root package, and standalone crates. Handles inherited versions (`version.workspace = true`). |
| **Semantic versioning** | `major`, `minor`, `patch`, and `none` bump types with configurable 0.x version semantics. |
| **Pre-release versions** | Built-in `alpha`, `beta`, `rc` tags and custom identifiers. Automatic counter increment (e.g., `alpha.1` → `alpha.2`). Graduation to stable by releasing without a pre-release flag. |
| **0.x → 1.0.0 graduation** | Promote pre-1.0 crates to stable via `--graduate` or a persistent graduation queue. |
| **Dependency-aware releases** | Automatically bumps workspace crates that depend on a released crate and generates changelog entries for the update. |
| **Keep a Changelog** | Generates [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/) entries grouped by category. Supports root or per-package changelogs with optional comparison links for GitHub, GitLab, Bitbucket, Gitea, Codeberg, and SourceHut. |
| **Git integration** | Creates commits and annotated tags automatically. Supports `version-only` (`v1.0.0`) and `crate-prefixed` (`crate@v1.0.0`) tag formats. |
| **CI-ready** | Auto-detects GitHub Actions, GitLab CI, CircleCI, Travis, Jenkins, Buildkite, and Azure DevOps. Ships as Docker images and pre-built binaries. Provides GitHub Actions for verify and release. |

<details>
<summary><strong>Instructions for AI Coding Agents 🤖</strong></summary>

If your project uses `cargo-changeset`, paste the following into your agent configuration file (e.g., `CLAUDE.md`, `AGENTS.md`, `.cursorrules`, or equivalent) so that your coding agent creates changesets automatically as part of its workflow.

---

````markdown
## Changeset Policy

This project uses `cargo-changeset` to manage versioning and changelogs.
Every code change that affects a published crate MUST include a changeset.
For detailed usage beyond what is covered here, run
`cargo changeset <command> --help`.

### When to add a changeset

Add a changeset whenever your changes affect the behavior, API, or
dependencies of one or more crates in this workspace. Do NOT add a changeset
for changes that are invisible to users of the crate, such as CI
configuration, documentation-only edits, or test-only refactors.

### How to add a changeset

Run the following command (no interactive prompts):

```bash
cargo changeset add \
  --package-bump <crate-name>:<major|minor|patch> \
  -m "<description>"
```

For changes affecting multiple crates, repeat `--package-bump` for each:

```bash
cargo changeset add \
  --package-bump crate-a:minor \
  --package-bump crate-b:patch \
  -m "<description>"
```

### Choosing the bump type

- `major` — breaking changes to the public API
- `minor` — new functionality that is backwards compatible
- `patch` — bug fixes and backwards-compatible corrections

### Writing the description

The changeset description appears in the CHANGELOG and is read by users of
the crate, not its developers. Write it from the perspective of someone who
depends on the crate and wants to know what changed and how it affects them.
Keep it to a single sentence when possible.

Good: "Add `--timeout` flag to control request deadline"
Good: "Fix panic when parsing empty configuration files"
Bad:  "Refactored the timeout module and added a CLI flag"
Bad:  "Fixed bug in config.rs line 42"

### Verifying coverage

After adding a changeset, verify that all affected crates are covered:

```bash
cargo changeset verify --base main
```

Exit code 0 means all changed crates have coverage.
````

---

</details>

## Installation

### From crates.io

```bash
cargo install cargo-changeset
```

Requires Rust **1.85.0** or later.

### Docker

For CI environments, pre-built Docker images are available for `linux/amd64` and `linux/arm64`:

| Registry | Image |
|----------|-------|
| GHCR | `ghcr.io/lukidoescode/cargo-changeset` |
| Docker Hub | `m3t4lukas/cargo-changeset` |

Tags: `latest` or a specific version (e.g., `0.1.2`). See [CI/CD Integration](#cicd-integration) for usage examples.

### Pre-built Binaries

Pre-compiled binaries are available on [GitHub Releases](https://github.com/lukidoescode/cargo-changeset/releases) for the following platforms:

| Platform | Target |
|----------|--------|
| Linux x86_64 | `x86_64-unknown-linux-musl` |
| Linux ARM64 | `aarch64-unknown-linux-musl` |
| macOS x86_64 | `x86_64-apple-darwin` |
| macOS ARM64 (Apple Silicon) | `aarch64-apple-darwin` |
| Windows x86_64 | `x86_64-pc-windows-msvc` |

Download the archive for your platform, extract the `cargo-changeset` binary, and place it somewhere on your `PATH`.

## Quick Start

> [!TIP]
> Every command supports `--help` for detailed usage information (e.g., `cargo changeset add --help`).

### Interactive and Non-Interactive Mode

`cargo-changeset` automatically detects whether it is running in an interactive terminal. When it is, commands that need user input will prompt for it. When it is not — because stdin is not a TTY, a CI environment variable is detected, or `CARGO_CHANGESET_NO_TTY` is set — interactive prompts are disabled entirely and all required information must be provided via flags.

Commands are also **partially interactive**: any information you pass as a flag is accepted as-is, and prompts are only shown for the remaining missing pieces. For example, `cargo changeset add --package my-crate` in a terminal will accept the package selection and still prompt you for the bump type, category, and description.

### 1. Initialize your project

Run `init` in the root of your Cargo project or workspace:

```bash
cargo changeset init
```

With no additional flags, this prompts you through configuring tag format, changelog location, version behavior, and other options step by step. If you pass specific flags (e.g., `--tag-format crate-prefixed`), only the remaining uncovered settings are prompted. Use `--defaults` to skip all prompts and accept the built-in defaults.

The command creates a `.changeset/` directory and writes the chosen configuration to `Cargo.toml` under `[workspace.metadata.changeset]` (or `[package.metadata.changeset]` for single-crate projects).

### 2. Add a changeset

After making changes to your code, record your intent to release:

```bash
cargo changeset add
```

With no flags, this prompts you through selecting packages, bump types, a changelog category, and a description. You can supply any combination of flags to skip the corresponding prompts — only missing information is asked for interactively.

To skip all prompts, provide everything on the command line:

```bash
cargo changeset add --package my-crate --bump minor --category added -m "Support custom templates"
```

For workspaces with multiple affected crates, use `--package-bump` to set bump types per crate:

```bash
cargo changeset add \
  --package-bump my-crate:minor \
  --package-bump my-other-crate:patch \
  -m "New feature in my-crate, fix in my-other-crate"
```

### 3. Preview pending changes

```bash
cargo changeset status
```

This shows all pending changesets and the projected version bumps for each crate.

### 4. Release

When you are ready to release:

```bash
cargo changeset release
```

This consumes all pending changesets, bumps versions in `Cargo.toml`, updates internal dependency versions, writes changelog entries, and creates a git commit with tags.

Use `--dry-run` to preview what would happen without modifying any files:

```bash
cargo changeset release --dry-run
```

## Commands

All commands accept the global `-C <PATH>` flag to set the project root directory. Run any command with `--help` for full usage details.

### `cargo changeset init`

Initialize the `.changeset/` directory and write configuration to `Cargo.toml`.

| Flag | Description |
|------|-------------|
| `--defaults` | Accept all defaults without prompts |
| `--no-interactive` | Disable prompts; use only CLI-provided values |
| `--tag-format <FORMAT>` | `version-only` or `crate-prefixed` |
| `--changelog <LOCATION>` | `root` or `per-package` |
| `--base-branch <BRANCH>` | Default base branch for comparisons (default: `main`) |
| `--zero-version-behavior <B>` | `effective-minor` or `auto-promote-on-major` |
| `--none-bump-behavior <B>` | `promote-to-patch`, `allow`, or `disallow` |

See `--help` for additional flags covering git, changelog templates, and file filtering options.

### `cargo changeset add`

Create a new changeset file.

| Flag | Description |
|------|-------------|
| `-p, --package <NAME>` | Package(s) to include (repeatable) |
| `-b, --bump <TYPE>` | Bump type for all selected packages (`major`, `minor`, `patch`, `none`) |
| `--package-bump <NAME:TYPE>` | Per-package bump type (repeatable) |
| `-c, --category <CAT>` | Change category (default: `changed`) |
| `-m, --message <TEXT>` | Description; use `-` to read from stdin |
| `--editor` | Open `$EDITOR` for the description |
| `--exclude-dependents` | Do not compute transitive dependents |

### `cargo changeset verify`

Check that all changed crates have changeset coverage.

| Flag | Description |
|------|-------------|
| `--base <BRANCH>` | Base branch to compare against (overrides config) |
| `--head <REF>` | Head ref to compare (default: `HEAD`) |
| `-q, --quiet` | Suppress output; exit code only |
| `--exclude-dependents` | Do not require coverage for transitive dependents |
| `--ignore-dirty` | Always compare against base branch, ignoring uncommitted changes |

### `cargo changeset status`

Show pending changesets and projected version bumps. No additional flags.

### `cargo changeset release`

Consume changesets and execute the release.

| Flag | Description |
|------|-------------|
| `--dry-run` | Preview changes without modifying files |
| `--convert` | Convert inherited versions (`version.workspace = true`) to explicit |
| `--no-commit` | Skip git commit; allows dirty working tree |
| `--no-tags` | Skip creating git tags |
| `--keep-changesets` | Do not delete changeset files after release |
| `--prerelease <CRATE:TAG>` | Create pre-release (e.g., `my-crate:alpha`); repeatable |
| `-f, --force` | Force release without changesets (pre-release increment only) |
| `--graduate <CRATE>` | Graduate a 0.x crate to 1.0.0; repeatable |

### `cargo changeset manage pre-release`

Manage persistent pre-release configurations stored in `.changeset/pre-release.toml`.

| Flag | Description |
|------|-------------|
| `--add <CRATE:TAG>` | Add a crate to pre-release (repeatable) |
| `--remove <CRATE>` | Remove a crate from pre-release (repeatable) |
| `--graduate <CRATE>` | Move a crate to the graduation queue (repeatable) |
| `-l, --list` | List current pre-release configuration |

### `cargo changeset manage graduation`

Manage the graduation queue for promoting 0.x crates to 1.0.0, stored in `.changeset/graduation.toml`.

| Flag | Description |
|------|-------------|
| `--add <CRATE>` | Add a 0.x crate to the graduation queue (repeatable) |
| `--remove <CRATE>` | Remove a crate from the queue (repeatable) |
| `-l, --list` | List crates marked for graduation |

## Configuration

Configuration is stored in `Cargo.toml` under `[workspace.metadata.changeset]` for workspaces or `[package.metadata.changeset]` for single-crate projects. All keys use kebab-case. Running `cargo changeset init` writes these values for you.

```toml
[workspace.metadata.changeset]
commit = true
tags = true
keep-changesets = false
tag-format = "crate-prefixed"
changelog = "root"
comparison-links = "auto"
zero-version-behavior = "effective-minor"
base-branch = "main"
none-bump-behavior = "promote-to-patch"
```

### Configuration Reference

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `commit` | bool | `true` | Create a git commit on release |
| `tags` | bool | `true` | Create git tags on release |
| `keep-changesets` | bool | `false` | Keep changeset files after release |
| `tag-format` | string | `"version-only"` | `"version-only"` (`v1.0.0`) or `"crate-prefixed"` (`crate@v1.0.0`) |
| `changelog` | string | `"root"` | `"root"` (single CHANGELOG.md) or `"per-package"` (one per crate) |
| `comparison-links` | string | `"auto"` | `"auto"`, `"enabled"`, or `"disabled"` |
| `zero-version-behavior` | string | `"effective-minor"` | `"effective-minor"` or `"auto-promote-on-major"` — see [Advanced Topics](#advanced-topics) |
| `base-branch` | string | `"main"` | Default base branch for `verify` comparisons |
| `none-bump-behavior` | string | `"promote-to-patch"` | `"promote-to-patch"`, `"allow"`, or `"disallow"` — see [Advanced Topics](#advanced-topics) |
| `none-bump-promote-message-template` | string | `"Internal architectural changes"` | Changelog message when `none` bumps are promoted to `patch` |
| `commit-title-template` | string | `"{new-version}"` | Template for the release commit title |
| `changes-in-body` | bool | `true` | Include version transitions in the commit body |
| `comparison-links-template` | string | _(auto-detected)_ | Custom URL template with `{repository}`, `{base}`, `{target}` placeholders |
| `dependency-bump-changelog-template` | string | ``"Updated dependency `{dependency}` to v{version}"`` | Template for auto-generated dependency bump changelog entries |
| `ignored-files` | array | `[]` | Glob patterns for files to ignore in change detection |

### Changeset File Format

Changeset files live in `.changeset/changesets/` and use YAML front matter with a Markdown body:

```markdown
---
category: fixed
my-crate: patch
my-other-crate: minor
---

Fixed authentication flow and added retry logic to the client.
```

The Markdown body after the closing `---` becomes the changelog entry. A single changeset can affect multiple crates, each with its own bump type.

**Front matter fields:**

| Field | Managed | Required | Default | Description |
|-------|---------|----------|---------|-------------|
| _crate-name_ | no | yes (at least one) | — | Maps a crate name to a bump type: `major`, `minor`, `patch`, or `none` |
| `category` | no | no | `changed` | One of `added`, `changed`, `deprecated`, `removed`, `fixed`, `security` |
| `graduate` | no | no | `false` | Set to `true` to graduate a 0.x crate to 1.0.0 on the next release — see [Advanced Topics](#advanced-topics) |
| `consumedForPrerelease` | yes | no | — | Written by the tool during pre-release builds. Records the pre-release version (e.g., `1.0.1-alpha.1`) this changeset was consumed in. Do not edit manually. |

## CI/CD Integration

`cargo-changeset` automatically detects CI environments by checking for these environment variables (in order): `CI`, `GITHUB_ACTIONS`, `GITLAB_CI`, `CIRCLECI`, `TRAVIS`, `JENKINS_URL`, `BUILDKITE`, `TF_BUILD`. When any of them is set, interactive prompts are disabled and all required input must be provided via flags.

Two environment variables allow explicit control, taking priority over the detection above:

| Variable | Priority | Description |
|----------|----------|-------------|
| `CARGO_CHANGESET_NO_TTY` | Highest | Disables interactive mode unconditionally, even if `CARGO_CHANGESET_FORCE_TTY` is also set |
| `CARGO_CHANGESET_FORCE_TTY` | Overrides CI detection | Enables interactive mode even when a CI environment variable is detected |

If neither override is set and no CI variable is found, the tool falls back to checking whether stdin is a terminal.

### GitHub Actions — Verify

The composite verify action checks changeset coverage on pull requests using the Docker image. No Rust toolchain or compilation is required.

```yaml
name: Verify Changeset Coverage

on:
  pull_request:
    branches: [main]

jobs:
  verify:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6
        with:
          fetch-depth: 0

      - uses: lukidoescode/cargo-changeset/.github/actions/verify@v1
        with:
          base: ${{ github.event.pull_request.base.ref }}
```

> [!IMPORTANT]
> `fetch-depth: 0` is required so that the full git history is available for comparing against the base branch.

The action fetches the base branch from origin and runs `verify --base "origin/<base>" --quiet` inside the Docker container.

**Inputs:**

| Input | Default | Description |
|-------|---------|-------------|
| `base` | `main` | Base branch to compare against |
| `cargo-changeset-version` | `latest` | Docker image tag to use |

### GitHub Actions — Release

The composite release action bumps versions, updates changelogs, and creates git commits and tags. It requires git user identity to be configured beforehand — the action validates this and fails with a clear error if it is missing.

```yaml
name: Release

on:
  workflow_dispatch:

jobs:
  release:
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - uses: actions/checkout@v6
        with:
          fetch-depth: 0

      - name: Configure git identity
        run: |
          git config user.name "github-actions[bot]"
          git config user.email "github-actions[bot]@users.noreply.github.com"

      - uses: lukidoescode/cargo-changeset/.github/actions/release@v1

      - name: Push changes and tags
        run: git push origin HEAD --tags
```

The action reads `user.name` and `user.email` from git config and passes them into the Docker container as `GIT_AUTHOR_NAME`, `GIT_AUTHOR_EMAIL`, `GIT_COMMITTER_NAME`, and `GIT_COMMITTER_EMAIL`. The action itself does **not** push — add a push step after it.

**Inputs:**

| Input | Default | Description |
|-------|---------|-------------|
| `dry-run` | `false` | Preview without modifying files |
| `convert` | `false` | Convert inherited versions to explicit |
| `no-commit` | `false` | Skip creating a release commit |
| `no-tags` | `false` | Skip creating git tags |
| `keep-changesets` | `false` | Keep changeset files after release |
| `force` | `false` | Force release without changesets |
| `prerelease` | — | Space-separated pre-release identifiers (e.g., `"foo:alpha bar:beta"`) |
| `graduate` | — | Space-separated crate names to graduate (e.g., `"foo bar"`) |
| `cargo-changeset-version` | `latest` | Docker image tag to use |

### GitHub Actions — Install from Source

If you prefer to install from source instead of using the Docker-based actions:

```yaml
- name: Install cargo-changeset
  run: cargo install cargo-changeset

- name: Verify changeset coverage
  run: cargo changeset verify --base "${{ github.event.pull_request.base.ref }}"
```

With caching to avoid reinstalling on every run:

```yaml
- name: Cache cargo-changeset
  uses: actions/cache@v5
  with:
    path: ~/.cargo/bin/cargo-changeset
    key: cargo-changeset-${{ runner.os }}-${{ hashFiles('**/Cargo.lock') }}

- name: Install cargo-changeset
  run: cargo install cargo-changeset

- name: Verify changeset coverage
  run: cargo changeset verify --base "${{ github.event.pull_request.base.ref }}"
```

### GitLab CI — Verify

Use the Docker image to verify changeset coverage on merge requests:

```yaml
verify-changesets:
  image: ghcr.io/lukidoescode/cargo-changeset:latest
  rules:
    - if: $CI_PIPELINE_SOURCE == "merge_request_event"
  script:
    - cargo-changeset verify --base "$CI_MERGE_REQUEST_TARGET_BRANCH_NAME" --quiet
```

### GitLab CI — Release

Use the Docker image to execute releases:

```yaml
release:
  image: ghcr.io/lukidoescode/cargo-changeset:latest
  rules:
    - if: $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH
      when: manual
  script:
    - git config user.name "gitlab-ci[bot]"
    - git config user.email "gitlab-ci[bot]@users.noreply.gitlab.com"
    - cargo-changeset release
    - git push origin "HEAD:$CI_COMMIT_BRANCH" --tags
```

> [!NOTE]
> Inside the Docker image, the binary is called `cargo-changeset` (single command with a hyphen), not `cargo changeset` (cargo subcommand). The entrypoint also automatically adds the mounted directory to git's `safe.directory` list.

## Git Hooks

### Manual Pre-Commit Hook

To enforce changeset coverage before every commit, add the following script as `.git/hooks/pre-commit` (or store it as `scripts/pre-commit` to commit it alongside your code):

```bash
#!/usr/bin/env bash
set -euo pipefail

BASE="${CHANGESET_BASE:-main}"

if ! command -v cargo-changeset &>/dev/null; then
  echo "error: cargo-changeset is not installed."
  echo "Install it with: cargo install cargo-changeset"
  exit 1
fi

cargo changeset verify --base "$BASE"
```

Make it executable and install it:

```bash
cp scripts/pre-commit .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit
```

The `--base` flag accepts a local branch name for local usage. Override the default base branch with the `CHANGESET_BASE` environment variable:

```bash
CHANGESET_BASE=develop git commit -m "my change"
```

### pre-commit Framework

If your project uses the [pre-commit framework](https://pre-commit.com), add a local hook to `.pre-commit-config.yaml`:

```yaml
repos:
  - repo: local
    hooks:
      - id: cargo-changeset
        name: Verify changeset coverage
        language: system
        entry: cargo changeset verify
        pass_filenames: false
        always_run: true
```

This calls the locally installed `cargo-changeset` binary. Install or update the hook with:

```bash
pre-commit install
```

## Advanced Topics

### Pre-Release Versions

`cargo-changeset` supports pre-release versions with built-in tags (`alpha`, `beta`, `rc`) and custom identifiers. Pre-release versions follow the semver format `X.Y.Z-tag.N` (e.g., `1.0.1-alpha.1`).

**Starting a pre-release** — pass `--prerelease` to the release command. If no changesets provide an explicit bump, the tool defaults to a patch bump:

```bash
cargo changeset release --prerelease my-crate:alpha
```

| Current Version | Bump | Flag | Result |
|-----------------|------|------|--------|
| `1.0.0` | patch | `--prerelease alpha` | `1.0.1-alpha.1` |
| `1.0.0` | minor | `--prerelease alpha` | `1.1.0-alpha.1` |
| `1.0.0` | major | `--prerelease alpha` | `2.0.0-alpha.1` |

**Incrementing a pre-release** — running with the same tag increments the counter:

| Current Version | Flag | Result |
|-----------------|------|--------|
| `1.0.1-alpha.1` | `--prerelease alpha` | `1.0.1-alpha.2` |
| `1.0.1-alpha.5` | `--prerelease alpha` | `1.0.1-alpha.6` |

**Switching tags** — changing the tag resets the counter to 1:

| Current Version | Flag | Result |
|-----------------|------|--------|
| `1.0.1-alpha.3` | `--prerelease beta` | `1.0.1-beta.1` |
| `1.0.1-beta.2` | `--prerelease rc` | `1.0.1-rc.1` |

**Graduating to stable** — release without `--prerelease` to strip the pre-release suffix:

| Current Version | Flag | Result |
|-----------------|------|--------|
| `1.0.1-rc.5` | _(none)_ | `1.0.1` |

For persistent pre-release configuration across multiple release cycles, use `cargo changeset manage pre-release` to store settings in `.changeset/pre-release.toml`.

### 0.x Version Handling

The `zero-version-behavior` configuration controls how semantic version bumps are interpreted for pre-1.0 crates.

**`effective-minor`** (default) — treats the minor version as the effective major version, dampening bumps by one level:

| Current | Bump | Result | Reasoning |
|---------|------|--------|-----------|
| `0.1.2` | major | `0.2.0` | Major → minor |
| `0.1.2` | minor | `0.1.3` | Minor → patch |
| `0.1.2` | patch | `0.1.3` | Patch stays patch |

**`auto-promote-on-major`** — a major bump on a 0.x version immediately promotes to 1.0.0:

| Current | Bump | Result | Reasoning |
|---------|------|--------|-----------|
| `0.1.2` | major | `1.0.0` | Promoted to stable |
| `0.1.2` | minor | `0.2.0` | Normal minor bump |
| `0.1.2` | patch | `0.1.3` | Normal patch bump |

### 0.x → 1.0.0 Graduation

To explicitly graduate a 0.x crate to 1.0.0, use the `--graduate` flag:

```bash
cargo changeset release --graduate my-crate
```

| Current | Flag | Result |
|---------|------|--------|
| `0.3.2` | `--graduate` | `1.0.0` |
| `0.5.3` | `--graduate --prerelease alpha` | `1.0.0-alpha.1` |

Graduation can only be applied to stable 0.x versions — crates that are already at 1.0.0 or higher, or that are currently in a pre-release, cannot be graduated.

For persistent graduation configuration, use `cargo changeset manage graduation` to queue crates in `.changeset/graduation.toml`. You can also set `graduate: true` in a changeset file to trigger graduation on the next release.

### Dependency-Aware Releases

In workspaces, when a crate is released, all crates that depend on it (through `[dependencies]` and `[build-dependencies]`) are automatically bumped with a **patch** version increment. This ensures that dependents always reference the latest version of their workspace siblings.

Auto-bumped crates receive a changelog entry under the "Changed" category using the `dependency-bump-changelog-template` (default: ``"Updated dependency `{dependency}` to v{version}"``). The `--exclude-dependents` flag on `add` and `verify` skips this dependency tracking.

Only workspace-internal dependencies are tracked — external crate dependencies are not affected.

### None Bump Type

The `none` bump type allows documenting changes in a changeset without incrementing the version. This is controlled by the `none-bump-behavior` configuration:

- **`promote-to-patch`** (default) — `none` bumps are silently promoted to `patch` on release, using the `none-bump-promote-message-template` as the changelog entry
- **`allow`** — `none` bumps are kept as-is; the changeset is consumed but no version change occurs
- **`disallow`** — changesets with `none` bumps are rejected

### Change Categories

Categories map directly to [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) sections and appear in the generated changelog in this order:

1. **Added** — new features
2. **Changed** — changes to existing functionality
3. **Deprecated** — features marked for removal
4. **Removed** — removed features
5. **Fixed** — bug fixes
6. **Security** — vulnerability fixes

The default category is `changed`. Set it with `--category` on `cargo changeset add` or via the `category` field in the changeset front matter.

## Contributing

Contributions are welcome. Please [open an issue](https://github.com/lukidoescode/cargo-changeset/issues) to report bugs or suggest features.

## License

Licensed under the [MIT License](LICENSE).
