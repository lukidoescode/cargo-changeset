# cargo-changeset

⚠️ **This project is in a very early stage of development and is not yet ready for production use.**

## What is cargo-changeset?

`cargo-changeset` is a tool for managing versioning and changelogs in Cargo projects and workspaces. It helps teams coordinate releases by allowing contributors to declare intent to release packages along with a summary of changes.

## How it works

1. **Add changesets**: When making changes, developers add a changeset file describing what changed and which packages should be released
2. **Track changes**: The tool tracks pending changesets and shows which packages need releases
3. **Bump versions**: When ready to release, `cargo-changeset` automatically bumps package versions according to semantic versioning rules
4. **Generate changelogs**: Changelog entries are automatically generated from changeset summaries

## CI Usage

### Automatic CI Detection

`cargo-changeset` automatically detects when it's running in a CI environment and disables interactive prompts. When CI is detected, commands that require user input will fail with a helpful error message explaining which flags to use.

The following CI environment variables are detected:
- `CI` (generic)
- `GITHUB_ACTIONS`
- `GITLAB_CI`
- `CIRCLECI`
- `TRAVIS`
- `JENKINS_URL`
- `BUILDKITE`
- `TF_BUILD` (Azure DevOps)

### Environment Variables

| Variable | Description |
|----------|-------------|
| `CARGO_CHANGESET_NO_TTY` | Disable interactive mode (highest priority) |
| `CARGO_CHANGESET_FORCE_TTY` | Force interactive mode (ignored in CI) |

### Non-Interactive Commands

When running in CI, provide all required arguments:

```bash
# Add a changeset non-interactively
cargo changeset add --package my-crate --bump minor -m "Added new feature"

# Add a changeset for multiple packages
cargo changeset add \
  --package-bump crate-a:major \
  --package-bump crate-b:patch \
  -m "Breaking change in crate-a, fix in crate-b"
```

---

## Integration

### Git Hook (Manual)

To enforce changeset coverage before every commit, add the following script to your repository:

**`.git/hooks/pre-commit`** (or `scripts/pre-commit` to commit alongside your code):

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

Then make it executable and install it:

```bash
cp scripts/pre-commit .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit
```

To override the default base branch, set the `CHANGESET_BASE` environment variable:

```bash
CHANGESET_BASE=develop git commit -m "my change"
```

### pre-commit Framework

If your project uses the [pre-commit framework](https://pre-commit.com), add a local hook to your `.pre-commit-config.yaml`:

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

This calls the locally-installed `cargo-changeset` binary directly. Install or update the hook with:

```bash
pre-commit install
```

### GitHub Actions CI

Add changeset verification to your pull request workflow to ensure every PR includes a changeset for any modified packages.

**Install and verify in a workflow:**

```yaml
- name: Install cargo-changeset
  run: cargo install cargo-changeset

- name: Verify changeset coverage
  run: cargo changeset verify --base ${{ github.event.pull_request.base.ref }}
```

**With caching to avoid reinstalling on every run:**

```yaml
- name: Cache cargo-changeset
  uses: actions/cache@v4
  with:
    path: ~/.cargo/bin/cargo-changeset
    key: cargo-changeset-${{ runner.os }}-${{ hashFiles('**/Cargo.lock') }}

- name: Install cargo-changeset
  run: cargo install cargo-changeset

- name: Verify changeset coverage
  run: cargo changeset verify --base ${{ github.event.pull_request.base.ref }}
```

**Complete workflow example (`.github/workflows/changeset.yml`):**

```yaml
name: Verify Changeset Coverage

on:
  pull_request:
    branches:
      - main

jobs:
  verify:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Cache cargo-changeset
        uses: actions/cache@v4
        with:
          path: ~/.cargo/bin/cargo-changeset
          key: cargo-changeset-${{ runner.os }}-${{ hashFiles('**/Cargo.lock') }}

      - name: Install cargo-changeset
        run: cargo install cargo-changeset

      - name: Verify changeset coverage
        run: cargo changeset verify --base ${{ github.event.pull_request.base.ref }}
```

---

### GitHub Actions Example (Adding Changesets)

```yaml
name: Add Changeset

on:
  workflow_dispatch:
    inputs:
      package:
        description: "Package to release"
        required: true
      bump:
        description: "Bump type (major, minor, patch)"
        required: true
        type: choice
        options:
          - patch
          - minor
          - major
      message:
        description: "Change description"
        required: true

jobs:
  add-changeset:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install cargo-changeset
        run: cargo install cargo-changeset

      - name: Add changeset
        run: |
          cargo changeset add \
            --package ${{ inputs.package }} \
            --bump ${{ inputs.bump }} \
            -m "${{ inputs.message }}"

      - name: Commit changeset
        run: |
          git config user.name "github-actions[bot]"
          git config user.email "github-actions[bot]@users.noreply.github.com"
          git add .changeset/
          git commit -m "Add changeset for ${{ inputs.package }}"
          git push
```
