# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.0.2] - 2026-02-26
### Added

- **cargo-changeset**: `--version` parameter on base command now prints version information

### Fixed

- **cargo-changeset**: Tags created with the crate-prefixed format now use @ as the separator (e.g., my-crate@v1.2.3) instead of - (e.g., my-crate-v1.2.3).
- **cargo-changeset**: Fix cargo subcommand dispatch by supporting both 'cargo changeset <cmd>' and direct 'cargo-changeset <cmd>' invocation modes
