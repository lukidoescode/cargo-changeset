use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("IO error")]
    Io(#[from] std::io::Error),

    #[error("TOML parse error")]
    TomlParse(#[from] toml::de::Error),

    #[error("no Cargo.toml found traversing from '{start_dir}'")]
    NotFound { start_dir: PathBuf },

    #[error("workspace member pattern '{pattern}' matched no directories")]
    NoMemberMatches { pattern: String },

    #[error("failed to read manifest at '{path}'")]
    ManifestRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse manifest at '{path}'")]
    ManifestParse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("manifest at '{path}' missing required field '{field}'")]
    MissingField { path: PathBuf, field: &'static str },

    #[error("invalid version '{version}' in package at '{path}'")]
    InvalidVersion {
        path: PathBuf,
        version: String,
        #[source]
        source: semver::Error,
    },

    #[error("failed to expand glob pattern '{pattern}'")]
    GlobPattern {
        pattern: String,
        #[source]
        source: glob::PatternError,
    },

    #[error("glob iteration error")]
    GlobIteration(#[from] glob::GlobError),
}
