use crate::{CommitInfo, GitError, Result};

use super::Repository;

impl Repository {
    /// Performs a soft reset to the parent of HEAD (HEAD~1).
    ///
    /// This undoes the last commit while keeping changes staged.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - HEAD cannot be resolved
    /// - HEAD has no parent (initial commit)
    /// - The reset operation fails
    pub fn reset_to_parent(&self) -> Result<()> {
        let head_commit = self.inner.head()?.peel_to_commit()?;
        let parent = head_commit
            .parent(0)
            .map_err(|source| GitError::NoParentCommit { source })?;
        self.inner
            .reset(parent.as_object(), git2::ResetType::Soft, None)?;
        Ok(())
    }

    /// # Errors
    ///
    /// Returns an error if the commit cannot be created.
    pub fn commit(&self, message: &str) -> Result<CommitInfo> {
        let sig = self.inner.signature()?;
        let mut index = self.inner.index()?;
        let tree_id = index.write_tree()?;
        let tree = self.inner.find_tree(tree_id)?;

        let parent = self.inner.head().ok().and_then(|h| h.peel_to_commit().ok());

        let parents: Vec<&git2::Commit<'_>> = parent.iter().collect();

        let commit_oid = self
            .inner
            .commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)?;

        let sha = commit_oid.to_string();

        Ok(CommitInfo {
            sha,
            message: message.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::setup_test_repo;
    use crate::GitError;
    use std::fs;
    use std::path::Path;

    #[test]
    fn create_commit() -> anyhow::Result<()> {
        let (dir, repo) = setup_test_repo()?;

        fs::write(dir.path().join("file.txt"), "content")?;
        repo.stage_files(&[Path::new("file.txt")])?;

        let commit_info = repo.commit("Test commit message")?;

        assert!(!commit_info.sha.is_empty());
        assert_eq!(commit_info.message, "Test commit message");

        let head = repo.inner.head()?.peel_to_commit()?;
        assert_eq!(head.id().to_string(), commit_info.sha);

        Ok(())
    }

    #[test]
    fn commit_with_multiline_message() -> anyhow::Result<()> {
        let (dir, repo) = setup_test_repo()?;

        fs::write(dir.path().join("file.txt"), "content")?;
        repo.stage_files(&[Path::new("file.txt")])?;

        let message = "Summary line\n\nDetailed description\nwith multiple lines";
        let commit_info = repo.commit(message)?;

        let head = repo.inner.head()?.peel_to_commit()?;
        assert_eq!(head.message(), Some(message));
        assert_eq!(commit_info.message, message);

        Ok(())
    }

    #[test]
    fn reset_to_parent_undoes_last_commit() -> anyhow::Result<()> {
        let (dir, repo) = setup_test_repo()?;

        let initial_head = repo.inner.head()?.peel_to_commit()?.id();

        fs::write(dir.path().join("file.txt"), "content")?;
        repo.stage_files(&[Path::new("file.txt")])?;
        repo.commit("Second commit")?;

        let after_commit_head = repo.inner.head()?.peel_to_commit()?.id();
        assert_ne!(initial_head, after_commit_head);

        repo.reset_to_parent()?;

        let after_reset_head = repo.inner.head()?.peel_to_commit()?.id();
        assert_eq!(initial_head, after_reset_head);

        Ok(())
    }

    #[test]
    fn reset_to_parent_on_initial_commit_fails() -> anyhow::Result<()> {
        let (_dir, repo) = setup_test_repo()?;

        let result = repo.reset_to_parent();

        assert!(result.is_err());
        let err = result.expect_err("expected NoParentCommit error");
        assert!(matches!(err, GitError::NoParentCommit { .. }));

        Ok(())
    }
}
