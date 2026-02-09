use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChangelogError {
    #[error("IO error")]
    Io(#[from] std::io::Error),

    #[error("failed to read changelog at '{path}'")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to write changelog at '{path}'")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse URL '{url}'")]
    UrlParse {
        url: String,
        #[source]
        source: url::ParseError,
    },

    #[error("invalid repository path in URL '{url}': expected owner/repo format")]
    InvalidRepositoryPath { url: String },

    #[error("invalid changelog format at '{path}': missing required header")]
    InvalidChangelogFormat { path: PathBuf },

    #[error("failed to parse version '{version}'")]
    VersionParse {
        version: String,
        #[source]
        source: semver::Error,
    },
}
