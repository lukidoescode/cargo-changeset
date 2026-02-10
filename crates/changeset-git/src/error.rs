use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GitError {
    #[error("git operation failed")]
    Git(#[from] git2::Error),

    #[error("not a git repository: '{path}'")]
    NotARepository { path: PathBuf },

    #[error("failed to resolve reference '{refspec}'")]
    RefNotFound { refspec: String },

    #[error("working tree has uncommitted changes")]
    DirtyWorkingTree,

    #[error("failed to delete file at '{path}'")]
    FileDelete {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("HEAD is detached, not on a branch")]
    DetachedHead,

    #[error("diff delta has no file path")]
    MissingDeltaPath,
}
