# Release with Changesets

A composite GitHub Action that runs `cargo changeset release` using the `cargo-changeset` Docker image. It bumps versions, updates changelogs, and creates release commits and tags based on pending changesets.

This action does **not** push commits or tags. You are responsible for pushing after the action completes.

## Prerequisites

- **Checkout with full history**: Use `actions/checkout@v6` with `fetch-depth: 0` so that git tags are available for version detection.
- **Git identity**: Configure `user.name` and `user.email` before running this action. The action will fail with an error if they are not set.

## Inputs

| Input | Description | Default |
|-------|-------------|---------|
| `dry-run` | Preview without modifying files | `false` |
| `convert` | Convert pre-release versions to stable | `false` |
| `no-commit` | Skip creating a release commit | `false` |
| `no-tags` | Skip creating git tags | `false` |
| `keep-changesets` | Keep changeset files after release | `false` |
| `force` | Force release even without changesets | `false` |
| `prerelease` | Pre-release identifiers, space-separated (e.g. `"foo:alpha bar:beta"`) | `""` |
| `graduate` | Graduate pre-releases to stable, space-separated crate names (e.g. `"foo bar"`) | `""` |
| `cargo-changeset-version` | Docker image tag for `cargo-changeset` | `"latest"` |

## Usage

```yaml
name: Release

on:
  workflow_dispatch:
    inputs:
      dry_run:
        description: "Dry run only"
        type: boolean
        default: false

permissions:
  contents: write

jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6
        with:
          fetch-depth: 0

      - name: Configure git identity
        run: |
          git config user.name "github-actions[bot]"
          git config user.email "github-actions[bot]@users.noreply.github.com"

      - uses: lukidoescode/cargo-changeset/.github/actions/release@main
        with:
          dry-run: ${{ inputs.dry_run }}

      - name: Push changes and tags
        if: ${{ !inputs.dry_run }}
        run: |
          git push
          git push --tags
```
