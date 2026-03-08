# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-03-08
### Added

- **changeset-operations**: Add operation types and interaction traits for prerelease and graduation management workflows.
- **changeset-operations**: Update intra-workspace dependency versions during release
- **cargo-changeset**: Update intra-workspace dependency versions during release
- **changeset-manifest**: Update intra-workspace dependency versions during release

### Changed

- **cargo-changeset**: Prerelease direct-add output now includes the configured tag name.
- **changeset-project**: CargoProject fields were made private.

### Fixed

- **changeset-operations**: Breaking: `Git2Provider::new()` now takes a `project_root` argument and returns `Result`. The provider validates all operations against this root and reuses the repository handle across calls instead of reopening it.
- **changeset-operations**: Propagate the real error when a remote URL cannot be resolved for comparison links instead of silently swallowing it.
- **changeset-operations**: Fix root changelog comparison link using the first package's version instead of the maximum version when releasing multiple packages simultaneously
- **changeset-operations**: Return an error when a changeset file cannot be read during release cleanup or backup instead of silently skipping it.
- **changeset-operations**: Fix error variant when changelog file read fails during release reporting it as a changeset file error
- **changeset-operations**: Preserve the full error chain for invalid prerelease tag and graduation constraint validation errors.
- **changeset-operations**: Use the highest current version across all released packages for the root changelog comparison link instead of whichever package appeared first.
- **cargo-changeset**: resolve remote tracking refs when resolving refspecs
- **cargo-changeset**: Fix root changelog comparison link using the first package's version instead of the maximum version when releasing multiple packages simultaneously
- **cargo-changeset**: Release failures during changeset cleanup now report the underlying cause instead of silently proceeding.
- **cargo-changeset**: Error messages for prerelease and graduation management are now more precise.
- **cargo-changeset**: Adapts to breaking API change in `changeset-operations`.
- **cargo-changeset**: Changelog comparison links now point to the correct version when releasing multiple packages.
- **changeset-git**: resolve remote tracking refs when resolving refspecs
- **changeset-git**: Detect Typechange deltas and error on missing old_path for renamed/copied files
- **changeset-changelog**: Return a dedicated `MissingHost` error when a remote URL has no host instead of a generic URL parse error.
- **changeset-changelog**: Fix root changelog comparison link using the first package's version instead of the maximum version when releasing multiple packages simultaneously

## [0.0.2] - 2026-02-26
### Added

- **cargo-changeset**: `--version` parameter on base command now prints version information

### Fixed

- **cargo-changeset**: Tags created with the crate-prefixed format now use @ as the separator (e.g., my-crate@v1.2.3) instead of - (e.g., my-crate-v1.2.3).
- **cargo-changeset**: Fix cargo subcommand dispatch by supporting both 'cargo changeset <cmd>' and direct 'cargo-changeset <cmd>' invocation modes
