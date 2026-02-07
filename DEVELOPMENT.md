# Development Guide

## Project Structure

This is a Cargo workspace containing multiple crates:

```
cargo-changeset/
├── crates/
│   ├── cargo-changeset/     # Main CLI executable
│   ├── changeset-core/      # Core types and traits
│   ├── changeset-parse/     # Changeset file parsing
│   ├── changeset-git/       # Git operations
│   └── changeset-version/   # Version bumping logic
```

### Crates

- **cargo-changeset**: The main CLI tool for managing changesets
- **changeset-core**: Core types, error handling, and shared functionality
- **changeset-parse**: Parser for changeset files
- **changeset-git**: Git operations (detecting changes, etc.)
- **changeset-version**: Semantic version bumping logic

## Building

```bash
cargo build
```

## Testing

```bash
cargo test
```

## Running the CLI

```bash
cargo run --bin cargo-changeset -- <command>
```

## Adding Dependencies

Workspace dependencies are managed in the root `Cargo.toml` under `[workspace.dependencies]`. When adding a new dependency:

1. Add it to the workspace dependencies section
2. Reference it in individual crate `Cargo.toml` files using `{ workspace = true }`

## Architecture

The project follows a modular architecture:

- **Core types** are defined in `changeset-core` and used across all crates
- **Parsing logic** is isolated in `changeset-parse`
- **Git operations** are handled by `changeset-git`
- **Version bumping** logic lives in `changeset-version`
- The **CLI** in `cargo-changeset` orchestrates these components
