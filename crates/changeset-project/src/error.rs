use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("IO error")]
    Io(#[from] std::io::Error),

    #[error("TOML parse error")]
    TomlParse(#[from] toml::de::Error),

    #[error("no Cargo.toml found traversing from '{start_dir}'")]
    NotFound { start_dir: PathBuf },

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

    #[error("invalid glob pattern '{pattern}'")]
    GlobPattern {
        pattern: String,
        #[source]
        source: globset::Error,
    },

    #[error("failed to parse glob pattern '{pattern}'")]
    GlobPatternParse {
        pattern: String,
        #[source]
        source: glob::PatternError,
    },

    #[error("glob traversal error")]
    GlobTraversal(#[from] glob::GlobError),

    #[error("path contains invalid UTF-8: '{path}'")]
    NonUtf8Path { path: PathBuf },

    #[error("failed to create directory '{path}'")]
    DirectoryCreate {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse JSON at '{path}'")]
    JsonParse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("failed to parse YAML at '{path}'")]
    YamlParse {
        path: PathBuf,
        #[source]
        source: serde_yml::Error,
    },

    #[error("version path '{version_field_path}' not found in external manifest at '{path}'")]
    ExternalVersionPathNotFound {
        path: PathBuf,
        version_field_path: String,
    },

    #[error(
        "expected string at version path '{version_field_path}' in external manifest at '{path}'"
    )]
    ExternalVersionNotString {
        path: PathBuf,
        version_field_path: String,
    },
}
