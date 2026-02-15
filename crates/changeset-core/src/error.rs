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
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PrereleaseSpecParseError {
    #[error("prerelease identifier cannot be empty")]
    Empty,

    #[error("prerelease identifier '{0}' contains invalid character '{1}'")]
    InvalidCharacter(String, char),
}

pub type Result<T> = std::result::Result<T, ChangesetError>;
