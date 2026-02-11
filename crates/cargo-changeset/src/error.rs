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

    #[error("format error")]
    Format(#[from] changeset_parse::FormatError),

    #[error("operation cancelled by user")]
    Cancelled,

    #[error("no packages found in project at '{0}'")]
    EmptyProject(PathBuf),

    #[error("interactive mode requires a terminal")]
    NotATty,

    #[error("internal error: single-package project has no packages")]
    ProjectInvariantViolation,

    #[error("unknown package '{name}' (available: {available})")]
    UnknownPackage { name: String, available: String },

    #[error("missing bump type for package '{package_name}' (use --bump or --package-bump)")]
    MissingBumpType { package_name: String },

    #[error("missing description (use -m or provide interactively)")]
    MissingDescription,

    #[error("description cannot be empty")]
    EmptyDescription,

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
}

pub type Result<T> = std::result::Result<T, CliError>;

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::CliError;

    #[test]
    fn empty_project_error_includes_path() {
        let err = CliError::EmptyProject(PathBuf::from("/my/project"));

        let msg = err.to_string();

        assert!(msg.contains("/my/project"));
    }

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
    fn cancelled_error_message() {
        let err = CliError::Cancelled;

        assert!(err.to_string().contains("cancelled"));
    }

    #[test]
    fn project_invariant_violation_message() {
        let err = CliError::ProjectInvariantViolation;

        let msg = err.to_string();

        assert!(msg.contains("internal error"));
        assert!(msg.contains("single-package"));
    }

    #[test]
    fn unknown_package_error_includes_name_and_available() {
        let err = CliError::UnknownPackage {
            name: "missing".to_string(),
            available: "foo, bar".to_string(),
        };

        let msg = err.to_string();

        assert!(msg.contains("missing"));
        assert!(msg.contains("foo, bar"));
    }

    #[test]
    fn missing_bump_type_error_includes_package_name() {
        let err = CliError::MissingBumpType {
            package_name: "my-package".to_string(),
        };

        let msg = err.to_string();

        assert!(msg.contains("my-package"));
        assert!(msg.contains("--bump"));
    }

    #[test]
    fn missing_description_error_message() {
        let err = CliError::MissingDescription;

        let msg = err.to_string();

        assert!(msg.contains("description"));
        assert!(msg.contains("-m"));
    }

    #[test]
    fn empty_description_error_message() {
        let err = CliError::EmptyDescription;

        assert!(err.to_string().contains("empty"));
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
    fn format_error_converts_via_from() {
        let format_err =
            changeset_parse::FormatError::from(changeset_parse::ValidationError::NoReleases);

        let cli_err: CliError = format_err.into();

        assert!(matches!(cli_err, CliError::Format(_)));
    }
}
