use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChangesetError {
    #[error("IO error")]
    Io(#[from] std::io::Error),

    #[error("failed to parse JSON")]
    JsonParse(#[from] serde_json::Error),

    #[error("failed to parse changeset file '{path}'")]
    ChangesetParse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("failed to parse version '{input}'")]
    VersionParse {
        input: String,
        #[source]
        source: semver::Error,
    },

    #[error("Git error: {0}")]
    Git(String),
}

pub type Result<T> = std::result::Result<T, ChangesetError>;
