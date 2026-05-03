# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-05-03
### Added

- **changeset-test-helpers**: Added helpers for sending Ctrl+C to the virtual terminal. Added helper for toggling items in interactive mode.
- **changeset-test-helpers**: Extracted end-to-end test helpers into `changeset-test-helpers` crate

### Changed

- **cargo-changeset**: Add additional-packages dependencies add/remove/list subcommands, and automatically patch-bump non-Rust packages whose version-tracking dependencies are released.
- **cargo-changeset**: Internal architectural changes
- **cargo-changeset**: Internal architectural changes
- **cargo-changeset**: Additional non-Rust packages are now discoverable and selectable via cargo changeset add, and tracked by cargo changeset verify
- **cargo-changeset**: Release command now versions additional non-Rust packages (Helm charts, etc.) alongside Cargo crates
- **cargo-changeset**: Add 'additional-packages' subcommand for managing non-Rust package declarations in Cargo.toml metadata; the subcommand requires a Cargo workspace and exits with a clear error in single-package projects
- **cargo-changeset**: Internal architectural changes
- **cargo-changeset**: Internal architectural changes
- **cargo-changeset**: Internal architectural changes
- **cargo-changeset**: Update crate keywords for better discoverability on crates.io
- **cargo-changeset**: Improve interactive ignored-files prompt in `init` command
- **cargo-changeset**: Internal architectural changes
- **changeset-operations**: Additional packages (Helm charts, Docker configs, etc.) now participate in verify and add operations via influence globs
- **changeset-operations**: Add operations, traits, and providers for managing additional (non-Rust) package declarations; additional-packages operations require a Cargo workspace and return a clear error for single-package projects
- **changeset-operations**: Internal architectural changes
- **changeset-operations**: Add ExternalManifestVersionWriter trait and discover_additional_packages to ProjectProvider for non-Rust package support
- **changeset-operations**: Expose public constructors for result types (`VerificationResult`, `ChangelogUpdate`, `CommitResult`, `TagResult`, `GitOperationResult`, `ReleaseOutput`)
- **changeset-operations**: Add non-Rust package support to the release saga, enabling version bumps of additional packages (YAML, JSON, TOML manifests) alongside Cargo crates
- **changeset-operations**: Internal architectural changes
- **changeset-operations**: Internal architectural changes
- **changeset-operations**: Add `WriteVersionTrackingStep` saga step that updates tracked external manifest fields during release, and config validation for circular, unknown, and duplicate version-tracking dependency declarations.
- **changeset-operations**: AddOperation::execute() now takes &AddInput instead of owned AddInput.
- **changeset-operations**: Include additional non-Rust packages in status output
- **changeset-operations**: Return full file path from write_changeset to fix incorrect path in add command output
- **changeset-manifest**: Add functions to read and write version-tracking dependencies in TOML, YAML, and JSON external manifests.
- **changeset-manifest**: Add functions for reading and writing non-Rust package declarations in Cargo.toml metadata
- **changeset-manifest**: Add support for writing and verifying versions in external non-Rust manifests in TOML, YAML, and JSON formats
- **changeset-core**: Add `VersionTrackingManifest` and `VersionTrackingDependency` types for declaring external manifest fields to update when a dependency's version changes.
- **changeset-core**: Internal architectural changes
- **changeset-core**: Add `ManifestFormat`, `AdditionalPackageManifest`, and `AdditionalPackageDeclaration` types for declaring non-Rust packages in workspace configuration
- **changeset-project**: Internal architectural changes
- **changeset-project**: Internal architectural changes
- **changeset-project**: Add `discover_additional_packages`, `compile_influence_patterns`, and `map_files_to_all_packages` for non-Rust package support
- **changeset-project**: Support declaring non-Rust packages via additional-packages in `Cargo.toml` workspace metadata; they are now available through `RootChangesetConfig`
- **changeset-project**: Add `add_members()` and `extend_with_edges()` to `WorkspaceDependencyGraph`, and `collect_version_tracking_info()` / `tracking_edges()` for integrating non-Rust packages into the dependency graph.
- **changeset-project**: Add support for reading versions from external non-Rust manifests in TOML, YAML, and JSON formats
- **changeset-changelog**: Internal architectural changes
- **changeset-changelog**: Internal architectural changes
- **changeset-parse**: Internal architectural changes
- **changeset-version**: Internal architectural changes
- **changeset-test-helpers**: Any timeout now prints the screen when tests fail
- **changeset-test-helpers**: Internal architectural changes
- **changeset-test-helpers**: Changed virtual terminal size to 1200x400

### Fixed

- **cargo-changeset**: Fixed typo in `README.md`
- **cargo-changeset**: Pressing ESC now aborts interactive mode instead of selecting the default
- **cargo-changeset**: Enforce `none_bump_behavior: disallow` during changeset creation, preventing `none` bumps in both interactive and non-interactive modes.
- **cargo-changeset**: Fix new changeset path in `add` command
- **changeset-operations**: Enforce `none_bump_behavior: disallow` during changeset creation

## [0.1.5] - 2026-03-30
### Changed

- **cargo-changeset**: Add code quality badges (security audit, coverage, MSRV) to README
- **cargo-changeset**: Add integration tests verifying lockfile stays current after release
- **cargo-changeset**: Use non-root user for Docker image with dynamic UID matching via su-exec

## [0.1.4] - 2026-03-30
### Fixed

- **changeset-operations**: Use cargo update --workspace instead of cargo generate-lockfile to preserve external dependency pins during release
- **cargo-changeset**: Fix Docker build failure by enabling MSRV-aware resolver v3 and decoupling Docker image from MSRV pin

## [0.1.3] - 2026-03-30
### Changed

- **cargo-changeset**: Upgrade cargo-changeset version in workflows from 0.1.0 to 0.1.2 and update outdated action versions (actions/checkout v4 to v6, actions/cache v4 to v5) in README examples
- **cargo-changeset**: Restructure README with improved organization, installation guide, quick start walkthrough, command reference, configuration reference, CI/CD integration examples, and pre-built agent instructions for seamless integration with AI coding assistants

## [0.1.2] - 2026-03-29
### Added

- **changeset-core**: Add `NoneBumpBehavior` type for configuring how `none` bump types are handled
- **cargo-changeset**: Add `--none-bump-behavior` and `--none-bump-promote-message-template` options to the `init` command
- **cargo-changeset**: Add CLI options to `cargo changeset init` for configuring commit templates, changelog templates, and file filtering
- **cargo-changeset**: Add `--base-branch` flag to init, prompt for it interactively, and automatically use the configured base branch in verify when `--base` is not provided
- **changeset-manifest**: Support configuring a default base branch via the base-branch field in the changeset metadata section
- **changeset-manifest**: Support reading and writing `none-bump-behavior` and `none-bump-promote-message-template` in `Cargo.toml` metadata
- **changeset-manifest**: Support reading and writing `dependency-bump-changelog-template` in changeset init config files
- **changeset-manifest**: Persist commit title, changelog templates, and file filtering configuration to `.changeset/config.toml`
- **changeset-git**: Export `DEFAULT_BASE_BRANCH` constant for consistent default base branch usage across crates
- **changeset-operations**: Add filtering settings support to initialization workflow with new `FilteringSettingsInput` type and `configure_filtering_settings()` trait method
- **changeset-operations**: Make the default base branch configurable via `GitSettingsInput`, propagated through all init paths into the written config
- **changeset-operations**: Automatically bump packages that depend on a changed package and generate changelog entries for the dependency update
- **changeset-operations**: Support promoting, allowing, or disallowing `none` bump types in release, status, and verify operations
- **changeset-project**: Expose `base_branch` on `RootChangesetConfig`, falling back to `DEFAULT_BASE_BRANCH` when not set in the manifest
- **changeset-project**: Add `dependency-bump-changelog-template` config option to customize the changelog message template for auto-bumped dependency updates
- **changeset-project**: Support `none-bump-behavior` and `none-bump-promote-message-template` configuration in workspace metadata

### Changed

- **changeset-core**: Internal architectural changes
- **cargo-changeset**: Internal architectural changes
- **cargo-changeset**: Use shared `DEFAULT_BASE_BRANCH` constant instead of hardcoded base branch strings
- **cargo-changeset**: Internal code reorganization following canonical file ordering rules
- **changeset-version**: Improve clarity of zero-version bump calculation logic
- **changeset-version**: Internal architectural changes
- **changeset-manifest**: Internal code reorganization following canonical file ordering rules
- **changeset-changelog**: Internal architectural changes
- **changeset-changelog**: Changelog entries are now generated for `none` bump changesets when promoted to `patch`
- **changeset-changelog**: Internal code reorganization following canonical file ordering rules
- **changeset-git**: Internal architectural changes
- **changeset-git**: Internal code reorganization following canonical file ordering rules
- **changeset-operations**: Internal architectural changes
- **changeset-operations**: Use shared `DEFAULT_BASE_BRANCH` constant instead of hardcoded base branch strings
- **changeset-operations**: Internal code reorganization following canonical file ordering rules
- **changeset-parse**: Internal architectural changes
- **changeset-parse**: Parsed changesets with `none` bump types can now be promoted to `patch` during release
- **changeset-parse**: Internal code reorganization following canonical file ordering rules
- **changeset-project**: Internal architectural changes
- **changeset-project**: Internal code reorganization following canonical file ordering rules
- **changeset-project**: Unify workspace and package config parsing into a single function and remove redundant serde rename attribute
- **changeset-saga**: Internal code reorganization following canonical file ordering rules

### Fixed

- **cargo-changeset**: Auto-bump dependent crates on release
- **cargo-changeset**: Faster and correct workspace discovery via fixed glob pattern matching
- **changeset-operations**: Faster and correct workspace discovery via fixed glob pattern matching
- **changeset-project**: Replace custom recursive directory walker with glob crate for correct and faster pattern matching

## [0.1.1] - 2026-03-15
### Added

- **changeset-parse**: Support parsing and serializing 'none' as a bump type in changeset files
- **cargo-changeset**: Warn about transitive workspace dependents not covered by a changeset when running "add"
- **cargo-changeset**: "verify" now detects uncommitted changes and verifies them against HEAD automatically
- **cargo-changeset**: Add 'none' bump type option for tracking changes without version increments
- **cargo-changeset**: Add "none" bump type for changesets that document changes without incrementing the version
- **cargo-changeset**: Add --exclude-dependents flag and show transitive dependent coverage in add, verify, and status commands
- **changeset-project**: Add WorkspaceDependencyGraph for resolving transitive workspace member dependencies
- **changeset-operations**: Add DependencyGraphProvider trait and dependency-aware logic to add, verify, and status operations
- **changeset-operations**: Warn about transitive workspace dependents not covered by a changeset when running "add"
- **changeset-operations**: "verify" now detects uncommitted changes and verifies them against HEAD automatically
- **changeset-operations**: Add "none" bump type for changesets that document changes without incrementing the version
- **changeset-operations**: Support non-bump changesets in add, status, verify, and release planning operations
- **changeset-version**: Support BumpType::None to preserve version unchanged during bump calculations
- **changeset-git**: "verify" now detects uncommitted changes and verifies them against HEAD automatically
- **changeset-core**: Add BumpType::None variant for tracking changes without version increments

### Changed

- **cargo-changeset**: "verify" output now distinguishes directly changed packages from transitive dependents
- **changeset-version**: Refactor version planner to track effective bump type separately from computed version

### Fixed

- **cargo-changeset**: Release now regenerates Cargo.lock automatically.
- **changeset-operations**: Cargo.lock is now automatically regenerated during release. If the release fails, the original lockfile is restored.

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

[0.1.1]: https://github.com/lukidoescode/cargo-changeset/compare/v0.1.0...v0.1.1

[0.1.2]: https://github.com/lukidoescode/cargo-changeset/compare/v0.1.1...v0.1.2

[0.1.3]: https://github.com/lukidoescode/cargo-changeset/compare/v0.1.2...v0.1.3

[0.1.4]: https://github.com/lukidoescode/cargo-changeset/compare/v0.1.3...v0.1.4

[0.1.5]: https://github.com/lukidoescode/cargo-changeset/compare/v0.1.4...v0.1.5

[0.2.0]: https://github.com/lukidoescode/cargo-changeset/compare/v0.1.5...v0.2.0
