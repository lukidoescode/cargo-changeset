use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChangesetError {
    #[error("IO error")]
    Io(#[from] std::io::Error),

    #[error("failed to parse version '{input}'")]
    VersionParse {
        input: String,
        #[source]
        source: semver::Error,
    },

    #[error(transparent)]
    Git(#[from] changeset_git::GitError),
}

pub type Result<T> = std::result::Result<T, ChangesetError>;
