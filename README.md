# cargo-changeset

⚠️ **This project is in a very early stage of development and is not yet ready for production use.**

## What is cargo-changeset?

`cargo-changeset` is a tool for managing versioning and changelogs in Cargo projects and workspaces. It helps teams coordinate releases by allowing contributors to declare intent to release packages along with a summary of changes.

## How it works

1. **Add changesets**: When making changes, developers add a changeset file describing what changed and which packages should be released
2. **Track changes**: The tool tracks pending changesets and shows which packages need releases
3. **Bump versions**: When ready to release, `cargo-changeset` automatically bumps package versions according to semantic versioning rules
4. **Generate changelogs**: Changelog entries are automatically generated from changeset summaries
