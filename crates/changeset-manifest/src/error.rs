use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("failed to read manifest at '{path}'")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to write manifest at '{path}'")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse TOML at '{path}'")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml_edit::TomlError,
    },

    #[error("missing required field '{field}' in '{path}'")]
    MissingField { path: PathBuf, field: String },

    #[error("expected version '{expected}' but found '{actual}' in '{path}'")]
    VerificationFailed {
        path: PathBuf,
        expected: String,
        actual: String,
    },

    #[error("invalid version string '{version}' in '{path}'")]
    InvalidVersion {
        path: PathBuf,
        version: String,
        #[source]
        source: semver::Error,
    },
}
