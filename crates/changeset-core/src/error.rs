use std::fmt;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestFormatParseError(pub(crate) String);

impl fmt::Display for ManifestFormatParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use clap::ValueEnum as _;
        let valid = crate::types::ManifestFormat::value_variants()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        write!(
            f,
            "unknown manifest format '{}', expected one of: {valid}",
            self.0
        )
    }
}

impl std::error::Error for ManifestFormatParseError {}

pub type Result<T> = std::result::Result<T, ChangesetError>;
