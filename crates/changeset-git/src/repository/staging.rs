use std::path::Path;

use crate::Result;

use super::Repository;

impl Repository {
    /// # Errors
    ///
    /// Returns an error if staging any of the files fails.
    pub fn stage_files(&self, paths: &[&Path]) -> Result<()> {
        let mut index = self.inner.index()?;

        for path in paths {
            let relative_path = self.to_relative_path(path);

            if path.exists() || self.root().join(&relative_path).exists() {
                index.add_path(&relative_path)?;
            } else {
                index.remove_path(&relative_path)?;
            }
        }

        index.write()?;
        Ok(())
    }

    /// # Errors
    ///
    /// Returns an error if the staging operation fails.
    pub fn stage_all(&self) -> Result<()> {
        let mut index = self.inner.index()?;

        index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
        index.write()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::setup_test_repo;
    use std::fs;
    use std::path::Path;

    #[test]
    fn stage_single_file() -> anyhow::Result<()> {
        let (dir, repo) = setup_test_repo()?;

        fs::write(dir.path().join("file.txt"), "content")?;

        repo.stage_files(&[Path::new("file.txt")])?;

        let index = repo.inner.index()?;
        assert!(index.get_path(Path::new("file.txt"), 0).is_some());

        Ok(())
    }

    #[test]
    fn stage_multiple_files() -> anyhow::Result<()> {
        let (dir, repo) = setup_test_repo()?;

        fs::write(dir.path().join("file1.txt"), "content1")?;
        fs::write(dir.path().join("file2.txt"), "content2")?;

        repo.stage_files(&[Path::new("file1.txt"), Path::new("file2.txt")])?;

        let index = repo.inner.index()?;
        assert!(index.get_path(Path::new("file1.txt"), 0).is_some());
        assert!(index.get_path(Path::new("file2.txt"), 0).is_some());

        Ok(())
    }

    #[test]
    fn stage_all_files() -> anyhow::Result<()> {
        let (dir, repo) = setup_test_repo()?;

        fs::write(dir.path().join("file1.txt"), "content1")?;
        fs::write(dir.path().join("file2.txt"), "content2")?;

        repo.stage_all()?;

        let index = repo.inner.index()?;
        assert!(index.get_path(Path::new("file1.txt"), 0).is_some());
        assert!(index.get_path(Path::new("file2.txt"), 0).is_some());

        Ok(())
    }

    #[test]
    fn stage_deleted_file() -> anyhow::Result<()> {
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

        fs::remove_file(dir.path().join("file.txt"))?;
        repo.stage_files(&[Path::new("file.txt")])?;

        let index = repo.inner.index()?;
        assert!(index.get_path(Path::new("file.txt"), 0).is_none());

        Ok(())
    }
}
