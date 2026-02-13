use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error(transparent)]
    Core(#[from] changeset_core::ChangesetError),

    #[error(transparent)]
    Git(#[from] changeset_git::GitError),

    #[error("project error")]
    Project(#[from] changeset_project::ProjectError),

    #[error("failed to determine current directory")]
    CurrentDir(#[source] std::io::Error),

    #[error("IO error")]
    Io(#[from] std::io::Error),

    #[error("operation error")]
    Operation(#[from] changeset_operations::OperationError),

    #[error("interactive mode requires a terminal")]
    NotATty,

    #[error("invalid --package-bump format '{input}' (expected 'package-name:bump-type')")]
    InvalidPackageBumpFormat { input: String },

    #[error("invalid bump type '{input}' (expected major, minor, or patch)")]
    InvalidBumpType { input: String },

    #[error("editor command failed")]
    EditorFailed {
        #[source]
        source: std::io::Error,
    },

    #[error("{uncovered_count} package(s) have changes without changeset coverage")]
    VerificationFailed { uncovered_count: usize },

    #[error(
        "changeset files were deleted in this branch (use --allow-deleted-changesets to bypass)"
    )]
    ChangesetDeleted { paths: Vec<PathBuf> },

    #[error("invalid prerelease tag '{tag}'")]
    InvalidPrereleaseTag { tag: String },
}

pub type Result<T> = std::result::Result<T, CliError>;

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::CliError;

    #[test]
    fn io_error_converts_via_from() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "test");

        let cli_err: CliError = io_err.into();

        assert!(matches!(cli_err, CliError::Io(_)));
    }

    #[test]
    fn project_error_converts_via_from() {
        let project_err = changeset_project::ProjectError::NotFound {
            start_dir: PathBuf::from("/test"),
        };

        let cli_err: CliError = project_err.into();

        assert!(matches!(cli_err, CliError::Project(_)));
    }

    #[test]
    fn project_error_has_source_chain() {
        let project_err = changeset_project::ProjectError::NotFound {
            start_dir: PathBuf::from("/test"),
        };
        let cli_err: CliError = project_err.into();

        let source = std::error::Error::source(&cli_err);

        assert!(source.is_some());
    }

    #[test]
    fn not_a_tty_error_message() {
        let err = CliError::NotATty;

        assert!(err.to_string().contains("terminal"));
    }

    #[test]
    fn invalid_package_bump_format_error_includes_input() {
        let err = CliError::InvalidPackageBumpFormat {
            input: "bad-format".to_string(),
        };

        let msg = err.to_string();

        assert!(msg.contains("bad-format"));
        assert!(msg.contains("package-name:bump-type"));
    }

    #[test]
    fn invalid_bump_type_error_includes_input() {
        let err = CliError::InvalidBumpType {
            input: "huge".to_string(),
        };

        let msg = err.to_string();

        assert!(msg.contains("huge"));
        assert!(msg.contains("major"));
    }

    #[test]
    fn editor_failed_error_has_source() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "editor not found");
        let err = CliError::EditorFailed { source: io_err };

        let source = std::error::Error::source(&err);

        assert!(source.is_some());
    }

    #[test]
    fn operation_error_converts_via_from() {
        let op_err = changeset_operations::OperationError::Cancelled;

        let cli_err: CliError = op_err.into();

        assert!(matches!(cli_err, CliError::Operation(_)));
    }
}
