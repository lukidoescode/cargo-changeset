use crate::{CommitInfo, Result};

use super::Repository;

impl Repository {
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
}
