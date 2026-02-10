use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error(transparent)]
    Core(#[from] changeset_core::ChangesetError),

    #[error(transparent)]
    Git(#[from] changeset_git::GitError),

    #[error("workspace error")]
    Workspace(#[from] changeset_workspace::WorkspaceError),

    #[error("IO error")]
    Io(#[from] std::io::Error),

    #[error("format error")]
    Format(#[from] changeset_parse::FormatError),

    #[error("operation cancelled by user")]
    Cancelled,

    #[error("no packages found in workspace at '{0}'")]
    EmptyWorkspace(PathBuf),

    #[error("interactive mode requires a terminal")]
    NotATty,

    #[error("internal error: single-crate workspace has no packages")]
    WorkspaceInvariantViolation,

    #[error("unknown crate '{name}' (available: {available})")]
    UnknownCrate { name: String, available: String },

    #[error("missing bump type for crate '{crate_name}' (use --bump or --crate-bump)")]
    MissingBumpType { crate_name: String },

    #[error("missing description (use -m or provide interactively)")]
    MissingDescription,

    #[error("description cannot be empty")]
    EmptyDescription,

    #[error("invalid --crate-bump format '{input}' (expected 'crate-name:bump-type')")]
    InvalidCrateBumpFormat { input: String },

    #[error("invalid bump type '{input}' (expected major, minor, or patch)")]
    InvalidBumpType { input: String },

    #[error("editor command failed")]
    EditorFailed {
        #[source]
        source: std::io::Error,
    },
}

pub type Result<T> = std::result::Result<T, CliError>;

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::CliError;

    #[test]
    fn empty_workspace_error_includes_path() {
        let err = CliError::EmptyWorkspace(PathBuf::from("/my/workspace"));

        let msg = err.to_string();

        assert!(msg.contains("/my/workspace"));
    }

    #[test]
    fn io_error_converts_via_from() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "test");

        let cli_err: CliError = io_err.into();

        assert!(matches!(cli_err, CliError::Io(_)));
    }

    #[test]
    fn workspace_error_converts_via_from() {
        let workspace_err = changeset_workspace::WorkspaceError::NotFound {
            start_dir: PathBuf::from("/test"),
        };

        let cli_err: CliError = workspace_err.into();

        assert!(matches!(cli_err, CliError::Workspace(_)));
    }

    #[test]
    fn workspace_error_has_source_chain() {
        let workspace_err = changeset_workspace::WorkspaceError::NotFound {
            start_dir: PathBuf::from("/test"),
        };
        let cli_err: CliError = workspace_err.into();

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
    fn workspace_invariant_violation_message() {
        let err = CliError::WorkspaceInvariantViolation;

        let msg = err.to_string();

        assert!(msg.contains("internal error"));
        assert!(msg.contains("single-crate"));
    }

    #[test]
    fn unknown_crate_error_includes_name_and_available() {
        let err = CliError::UnknownCrate {
            name: "missing".to_string(),
            available: "foo, bar".to_string(),
        };

        let msg = err.to_string();

        assert!(msg.contains("missing"));
        assert!(msg.contains("foo, bar"));
    }

    #[test]
    fn missing_bump_type_error_includes_crate_name() {
        let err = CliError::MissingBumpType {
            crate_name: "my-crate".to_string(),
        };

        let msg = err.to_string();

        assert!(msg.contains("my-crate"));
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
    fn invalid_crate_bump_format_error_includes_input() {
        let err = CliError::InvalidCrateBumpFormat {
            input: "bad-format".to_string(),
        };

        let msg = err.to_string();

        assert!(msg.contains("bad-format"));
        assert!(msg.contains("crate-name:bump-type"));
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
