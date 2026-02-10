use std::path::Path;

use crate::{GitError, Result};

use super::Repository;

impl Repository {
    /// # Errors
    ///
    /// Returns [`GitError::FileDelete`] if the file cannot be deleted.
    pub fn delete_file(&self, path: &Path) -> Result<()> {
        let absolute_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root().join(path)
        };

        std::fs::remove_file(&absolute_path).map_err(|source| GitError::FileDelete {
            path: absolute_path,
            source,
        })?;

        let relative_path = self.to_relative_path(path);

        let mut index = self.inner.index()?;
        index.remove_path(relative_path)?;
        index.write()?;

        Ok(())
    }

    /// # Errors
    ///
    /// Returns [`GitError::FileDelete`] if any file cannot be deleted.
    pub fn delete_files(&self, paths: &[&Path]) -> Result<()> {
        for path in paths {
            self.delete_file(path)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::setup_test_repo;
    use crate::GitError;
    use std::fs;
    use std::path::Path;

    #[test]
    fn delete_single_file() -> anyhow::Result<()> {
        let (dir, repo) = setup_test_repo()?;

        fs::write(dir.path().join("file.txt"), "content")?;
        repo.stage_files(&[Path::new("file.txt")])?;

        let sig = git2::Signature::now("Test", "test@example.com")?;
        let mut index = repo.inner.index()?;
        let tree_id = index.write_tree()?;
        let tree = repo.inner.find_tree(tree_id)?;
        let parent = repo.inner.head()?.peel_to_commit()?;
        repo.inner
            .commit(Some("HEAD"), &sig, &sig, "Add file", &tree, &[&parent])?;

        repo.delete_file(Path::new("file.txt"))?;

        assert!(!dir.path().join("file.txt").exists());

        let index = repo.inner.index()?;
        assert!(index.get_path(Path::new("file.txt"), 0).is_none());

        Ok(())
    }

    #[test]
    fn delete_multiple_files() -> anyhow::Result<()> {
        let (dir, repo) = setup_test_repo()?;

        fs::write(dir.path().join("file1.txt"), "content1")?;
        fs::write(dir.path().join("file2.txt"), "content2")?;
        repo.stage_files(&[Path::new("file1.txt"), Path::new("file2.txt")])?;

        let sig = git2::Signature::now("Test", "test@example.com")?;
        let mut index = repo.inner.index()?;
        let tree_id = index.write_tree()?;
        let tree = repo.inner.find_tree(tree_id)?;
        let parent = repo.inner.head()?.peel_to_commit()?;
        repo.inner
            .commit(Some("HEAD"), &sig, &sig, "Add files", &tree, &[&parent])?;

        repo.delete_files(&[Path::new("file1.txt"), Path::new("file2.txt")])?;

        assert!(!dir.path().join("file1.txt").exists());
        assert!(!dir.path().join("file2.txt").exists());

        Ok(())
    }

    #[test]
    fn delete_nonexistent_file_fails() -> anyhow::Result<()> {
        let (_dir, repo) = setup_test_repo()?;

        let result = repo.delete_file(Path::new("nonexistent.txt"));
        assert!(matches!(result, Err(GitError::FileDelete { .. })));

        Ok(())
    }
}
