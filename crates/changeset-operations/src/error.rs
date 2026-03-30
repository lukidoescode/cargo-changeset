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

    #[error("failed to read changelog file '{path}'")]
    ChangelogFileRead {
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

    #[error("project root mismatch: provider configured for '{}' but called with '{}'", expected.display(), actual.display())]
    ProjectRootMismatch { expected: PathBuf, actual: PathBuf },

    #[error("failed to canonicalize project root '{}'", path.display())]
    ProjectRootCanonicalize {
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

    #[error("changesets with bump type 'none' are disallowed; affected packages: {}", packages.join(", "))]
    NoneBumpDisallowed { packages: Vec<String> },

    #[error("comparison links enabled but no repository URL available")]
    ComparisonLinksRequired,

    #[error("comparison links enabled but repository URL could not be parsed")]
    ComparisonLinksUrlParse(#[source] changeset_changelog::ChangelogError),

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
    VersionParse {
        version: String,
        context: String,
        #[source]
        source: semver::Error,
    },

    #[error("failed to delete {} tag(s) during compensation: {}", failed_tags.len(), failed_tags.join(", "))]
    TagDeletionFailed { failed_tags: Vec<String> },

    #[error("package '{name}' not found in workspace")]
    PackageNotFound { name: String },

    #[error("cannot graduate package '{package}' with prerelease version '{version}'")]
    CannotGraduatePrerelease {
        package: String,
        version: semver::Version,
    },

    #[error("cannot graduate package '{package}' with stable version '{version}' (>= 1.0.0)")]
    CannotGraduateStable {
        package: String,
        version: semver::Version,
    },

    #[error("invalid pre-release format '{input}' (expected 'crate:tag')")]
    InvalidPrereleaseFormat { input: String },

    #[error("invalid prerelease tag '{tag}'")]
    InvalidPrereleaseTag {
        tag: String,
        #[source]
        source: changeset_core::PrereleaseSpecParseError,
    },

    #[error("failed to read lockfile '{}'", path.display())]
    LockfileRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to write lockfile '{}'", path.display())]
    LockfileWrite {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("lockfile generation failed in '{}'", path.display())]
    LockfileGeneration {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("cargo update --workspace failed: {stderr}")]
    LockfileCommandFailed { stderr: String },

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
            other => Self::SagaFailed {
                step: other.to_string(),
                source: Box::new(Self::Cancelled),
            },
        }
    }
}

/// # Errors
///
/// Returns [`OperationError::InvalidPrereleaseTag`] when `tag` cannot be parsed.
pub fn parse_prerelease_tag(tag: &str) -> Result<changeset_core::PrereleaseSpec> {
    tag.parse()
        .map_err(|source| OperationError::InvalidPrereleaseTag {
            tag: tag.to_string(),
            source,
        })
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

    #[test]
    fn project_root_canonicalize_error_includes_path() {
        let err = OperationError::ProjectRootCanonicalize {
            path: PathBuf::from("/some/path"),
            source: std::io::Error::other("test"),
        };
        assert!(err.to_string().contains("/some/path"));
    }

    #[test]
    fn version_parse_error_includes_version_and_context() {
        let err = OperationError::VersionParse {
            version: "not-a-version".to_string(),
            context: "test context".to_string(),
            source: "bad".parse::<semver::Version>().expect_err("should fail"),
        };
        assert!(err.to_string().contains("not-a-version"));
        assert!(err.to_string().contains("test context"));
    }

    #[test]
    fn parse_prerelease_tag_succeeds_for_valid_tag() {
        let spec = parse_prerelease_tag("alpha").expect("should parse valid tag");

        assert_eq!(spec.identifier(), "alpha");
    }

    #[test]
    fn parse_prerelease_tag_returns_error_for_invalid_tag() {
        let err = parse_prerelease_tag("bad.tag").expect_err("should fail for invalid tag");

        assert!(matches!(
            err,
            OperationError::InvalidPrereleaseTag { ref tag, .. } if tag == "bad.tag"
        ));
    }
}
