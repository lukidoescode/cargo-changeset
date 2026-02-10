use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error(transparent)]
    Core(#[from] changeset_core::ChangesetError),

    #[error(transparent)]
    Git(#[from] changeset_git::GitError),
}

pub type Result<T> = std::result::Result<T, CliError>;
