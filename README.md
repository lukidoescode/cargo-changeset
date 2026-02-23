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

### GitHub Actions Example

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
