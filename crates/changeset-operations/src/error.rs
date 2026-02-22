use std::path::PathBuf;

use changeset_saga::SagaError;
use thiserror::Error;

/// Details about a failed compensation during saga rollback.
#[derive(Debug)]
pub struct CompensationFailure {
    /// Name of the step whose compensation failed.
    pub step: String,
    /// Description of what the compensation was trying to do.
    pub description: String,
    /// The error that occurred during compensation.
    pub error: Box<OperationError>,
}

#[derive(Debug, Error)]
pub enum OperationError {
    #[error(transparent)]
    Core(#[from] changeset_core::ChangesetError),

    #[error(transparent)]
    Git(#[from] changeset_git::GitError),

    #[error(transparent)]
    Project(#[from] changeset_project::ProjectError),

    #[error(transparent)]
    Parse(#[from] changeset_parse::FormatError),

    #[error(transparent)]
    Manifest(#[from] changeset_manifest::ManifestError),

    #[error(transparent)]
    Changelog(#[from] changeset_changelog::ChangelogError),

    #[error("version calculation failed")]
    VersionCalculation(#[from] changeset_version::VersionError),

    #[error("failed to read changeset file '{path}'")]
    ChangesetFileRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse changeset file '{path}'")]
    ChangesetParse {
        path: PathBuf,
        #[source]
        source: changeset_parse::FormatError,
    },

    #[error("failed to write changeset file")]
    ChangesetFileWrite(#[source] std::io::Error),

    #[error("failed to list changeset files in '{path}'")]
    ChangesetList {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("operation cancelled")]
    Cancelled,

    #[error("no packages found in project at '{0}'")]
    EmptyProject(PathBuf),

    #[error("unknown package '{name}' (available: {available})")]
    UnknownPackage { name: String, available: String },

    #[error("missing bump type for package '{package_name}'")]
    MissingBumpType { package_name: String },

    #[error("missing description")]
    MissingDescription,

    #[error("description cannot be empty")]
    EmptyDescription,

    #[error("no packages selected")]
    NoPackagesSelected,

    #[error("interaction required but provider returned None")]
    InteractionRequired,

    #[error("IO error")]
    Io(#[from] std::io::Error),

    #[error("packages with inherited versions require --convert flag: {}", packages.join(", "))]
    InheritedVersionsRequireConvert { packages: Vec<String> },

    #[error("comparison links enabled but no repository URL available")]
    ComparisonLinksRequired,

    #[error("working tree has uncommitted changes; commit or stash them, or use --no-commit")]
    DirtyWorkingTree,

    #[error("current version is stable; please specify a pre-release tag: --prerelease <tag>")]
    PrereleaseTagRequired,

    #[error("no changesets found; use --force to release without changesets")]
    NoChangesetsWithoutForce,

    #[error("invalid changeset path '{path}': {reason}")]
    InvalidChangesetPath { path: PathBuf, reason: &'static str },

    #[error("failed to read release state file '{path}'")]
    ReleaseStateRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to write release state file '{path}'")]
    ReleaseStateWrite {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse release state file '{path}'")]
    ReleaseStateParse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("failed to serialize release state for '{path}'")]
    ReleaseStateSerialize {
        path: PathBuf,
        #[source]
        source: toml::ser::Error,
    },

    #[error("release validation failed")]
    ValidationFailed(#[from] crate::operations::ValidationErrors),

    #[error("failed to parse version '{version}' during {context}")]
    VersionParse { version: String, context: String },

    #[error("failed to delete {} tag(s) during compensation: {}", failed_tags.len(), failed_tags.join(", "))]
    TagDeletionFailed { failed_tags: Vec<String> },

    #[error("release saga failed at step '{step}'")]
    SagaFailed {
        step: String,
        #[source]
        source: Box<OperationError>,
    },

    #[error(
        "release saga failed at step '{step}' and {} compensation(s) also failed", compensation_failures.len()
    )]
    SagaCompensationFailed {
        step: String,
        source: Box<OperationError>,
        compensation_failures: Vec<CompensationFailure>,
    },
}

pub type Result<T> = std::result::Result<T, OperationError>;

impl From<SagaError<OperationError>> for OperationError {
    fn from(err: SagaError<OperationError>) -> Self {
        match err {
            SagaError::StepFailed { step, source } => Self::SagaFailed {
                step,
                source: Box::new(source),
            },
            SagaError::CompensationFailed {
                failed_step,
                step_error,
                compensation_errors,
            } => {
                let compensation_failures = compensation_errors
                    .into_iter()
                    .map(|e| CompensationFailure {
                        step: e.step,
                        description: e.description,
                        error: Box::new(e.error),
                    })
                    .collect();
                Self::SagaCompensationFailed {
                    step: failed_step,
                    source: Box::new(step_error),
                    compensation_failures,
                }
            }
            _ => Self::SagaFailed {
                step: "unknown".to_string(),
                source: Box::new(Self::Cancelled),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_project_error_includes_path() {
        let err = OperationError::EmptyProject(PathBuf::from("/my/project"));

        let msg = err.to_string();

        assert!(msg.contains("/my/project"));
    }

    #[test]
    fn unknown_package_error_includes_name_and_available() {
        let err = OperationError::UnknownPackage {
            name: "missing".to_string(),
            available: "foo, bar".to_string(),
        };

        let msg = err.to_string();

        assert!(msg.contains("missing"));
        assert!(msg.contains("foo, bar"));
    }

    #[test]
    fn cancelled_error_message() {
        let err = OperationError::Cancelled;

        assert!(err.to_string().contains("cancelled"));
    }
}
