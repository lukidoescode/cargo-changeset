use std::path::PathBuf;

use thiserror::Error;

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
}

pub type Result<T> = std::result::Result<T, OperationError>;

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
