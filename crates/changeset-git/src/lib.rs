mod error;
mod repository;
mod types;

pub use error::GitError;
pub use repository::Repository;
pub use types::{CommitInfo, FileChange, FileStatus, TagInfo};

use std::path::Path;

pub type Result<T> = std::result::Result<T, GitError>;

/// # Errors
///
/// Returns an error if the path is not a git repository or if the status check fails.
pub fn is_working_tree_clean(path: &Path) -> Result<bool> {
    Repository::open(path)?.is_working_tree_clean()
}

/// # Errors
///
/// Returns an error if the path is not a git repository or if HEAD is detached.
pub fn current_branch(path: &Path) -> Result<String> {
    Repository::open(path)?.current_branch()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::tests::setup_test_repo;
    use std::fs;

    #[test]
    fn is_working_tree_clean_via_public_fn() -> anyhow::Result<()> {
        let (dir, _repo) = setup_test_repo()?;

        assert!(is_working_tree_clean(dir.path())?);

        fs::write(dir.path().join("untracked.txt"), "content")?;
        assert!(!is_working_tree_clean(dir.path())?);

        Ok(())
    }

    #[test]
    fn current_branch_via_public_fn() -> anyhow::Result<()> {
        let (dir, _repo) = setup_test_repo()?;
        let branch = current_branch(dir.path())?;
        assert!(branch == "main" || branch == "master");
        Ok(())
    }
}
