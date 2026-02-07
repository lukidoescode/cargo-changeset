use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChangesetError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Git error: {0}")]
    Git(String),

    #[error("Version error: {0}")]
    Version(String),
}

pub type Result<T> = std::result::Result<T, ChangesetError>;
