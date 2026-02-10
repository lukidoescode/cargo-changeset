use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("workspace error")]
    Workspace(#[from] changeset_workspace::WorkspaceError),

    #[error("IO error")]
    Io(#[from] std::io::Error),

    #[error("operation cancelled by user")]
    Cancelled,

    #[error("no packages found in workspace at '{0}'")]
    EmptyWorkspace(PathBuf),

    #[error("interactive mode requires a terminal")]
    NotATty,

    #[error("internal error: single-crate workspace has no packages")]
    WorkspaceInvariantViolation,
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
}
