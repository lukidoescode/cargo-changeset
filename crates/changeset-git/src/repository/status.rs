use crate::{GitError, Result};

use super::Repository;

impl Repository {
    /// # Errors
    ///
    /// Returns [`GitError::DetachedHead`] if HEAD is not on a branch.
    pub fn current_branch(&self) -> Result<String> {
        let head = self.inner.head()?;

        if !head.is_branch() {
            return Err(GitError::DetachedHead);
        }

        head.shorthand()
            .map(String::from)
            .ok_or(GitError::DetachedHead)
    }

    /// # Errors
    ///
    /// Returns an error if the git status operation fails.
    pub fn is_working_tree_clean(&self) -> Result<bool> {
        let statuses = self.inner.statuses(Some(
            git2::StatusOptions::new()
                .include_untracked(true)
                .recurse_untracked_dirs(true),
        ))?;

        Ok(statuses.is_empty())
    }

    /// # Errors
    ///
    /// Returns [`GitError::DirtyWorkingTree`] if there are uncommitted changes.
    pub fn require_clean_working_tree(&self) -> Result<()> {
        if self.is_working_tree_clean()? {
            Ok(())
        } else {
            Err(GitError::DirtyWorkingTree)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::setup_test_repo;
    use std::fs;

    #[test]
    fn current_branch_on_main() -> anyhow::Result<()> {
        let (_dir, repo) = setup_test_repo()?;
        let branch = repo.current_branch()?;
        assert!(branch == "main" || branch == "master");
        Ok(())
    }

    #[test]
    fn clean_working_tree() -> anyhow::Result<()> {
        let (_dir, repo) = setup_test_repo()?;
        assert!(repo.is_working_tree_clean()?);
        Ok(())
    }

    #[test]
    fn dirty_working_tree_with_untracked_file() -> anyhow::Result<()> {
        let (dir, repo) = setup_test_repo()?;
        fs::write(dir.path().join("new_file.txt"), "content")?;
        assert!(!repo.is_working_tree_clean()?);
        Ok(())
    }

    #[test]
    fn require_clean_fails_on_dirty() -> anyhow::Result<()> {
        let (dir, repo) = setup_test_repo()?;
        fs::write(dir.path().join("new_file.txt"), "content")?;

        let result = repo.require_clean_working_tree();
        assert!(result.is_err());
        Ok(())
    }
}
