# Verify Changeset Coverage

A composite GitHub Action that runs `cargo changeset verify` using the `cargo-changeset` Docker image. It checks that all crates with changes have corresponding changeset files.

## Prerequisites

- **Checkout**: Use `actions/checkout@v6` so the repository is available.
- **Git fetch**: The action automatically fetches the base branch from `origin` and compares against `origin/<base>`. No manual fetch step is needed.

## Inputs

| Input | Description | Default |
|-------|-------------|---------|
| `base` | Base branch to compare against | `"main"` |
| `cargo-changeset-version` | Docker image tag for `cargo-changeset` | `"latest"` |

## Usage

```yaml
name: Verify Changesets

on:
  pull_request:
    branches: [main]

permissions:
  contents: read

jobs:
  verify:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6
        with:
          fetch-depth: 0

      - uses: lukidoescode/cargo-changeset/.github/actions/verify@main
        with:
          base: main
```
