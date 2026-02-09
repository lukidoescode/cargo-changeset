mod error;
mod manifest;
mod workspace;

pub use error::WorkspaceError;
pub use workspace::{
    Workspace, WorkspaceKind, discover_workspace, discover_workspace_from_cwd, ensure_changeset_dir,
};

pub type Result<T> = std::result::Result<T, WorkspaceError>;
