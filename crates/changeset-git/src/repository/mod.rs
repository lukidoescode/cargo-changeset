mod commit;
mod diff;
mod files;
mod remote;
mod staging;
mod status;
mod tag;

use std::path::{Path, PathBuf};

use crate::{GitError, Result};

pub struct Repository {
    pub(crate) inner: git2::Repository,
    root: PathBuf,
}

impl Repository {
    /// # Errors
    ///
    /// Returns [`GitError::NotARepository`] if the path is not inside a git repository.
    pub fn open(path: &Path) -> Result<Self> {
        let inner = git2::Repository::discover(path).map_err(|_| GitError::NotARepository {
            path: path.to_path_buf(),
        })?;

        let root = inner.workdir().ok_or_else(|| GitError::NotARepository {
            path: path.to_path_buf(),
        })?;

        // Use dunce to get a path without the \\?\ prefix on Windows
        let root = dunce::simplified(root).to_path_buf();

        Ok(Self { inner, root })
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn to_relative_path(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            // Use dunce to normalize the path (removes \\?\ prefix on Windows)
            let normalized = dunce::simplified(path);
            normalized
                .strip_prefix(&self.root)
                .map_or_else(|_| path.to_path_buf(), Path::to_path_buf)
        } else {
            path.to_path_buf()
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use tempfile::TempDir;

    pub(crate) fn setup_test_repo() -> anyhow::Result<(TempDir, Repository)> {
        let dir = TempDir::new()?;
        let repo = git2::Repository::init(dir.path())?;

        let mut config = repo.config()?;
        config.set_str("user.name", "Test")?;
        config.set_str("user.email", "test@example.com")?;

        let sig = git2::Signature::now("Test", "test@example.com")?;
        let tree_id = repo.index()?.write_tree()?;
        let tree = repo.find_tree(tree_id)?;
        repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])?;

        let repository = Repository::open(dir.path())?;
        Ok((dir, repository))
    }

    #[test]
    fn open_repository() -> anyhow::Result<()> {
        let (dir, repo) = setup_test_repo()?;
        let expected = dir.path().canonicalize()?;
        let actual = repo.root().canonicalize()?;
        assert_eq!(actual, expected);
        Ok(())
    }

    #[test]
    fn open_nonexistent_repository() {
        let dir = TempDir::new().expect("failed to create temp dir");
        let result = Repository::open(dir.path());
        assert!(matches!(result, Err(GitError::NotARepository { .. })));
    }
}
