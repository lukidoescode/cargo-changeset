use std::path::PathBuf;

use crate::{FileChange, FileStatus, GitError, Result};

use super::Repository;

impl Repository {
    /// # Errors
    ///
    /// Returns [`GitError::RefNotFound`] if either base or head cannot be resolved.
    pub fn changed_files(&self, base: Option<&str>, head: &str) -> Result<Vec<FileChange>> {
        let head_tree = self.resolve_tree(head)?;

        let base_tree = base.map(|refspec| self.resolve_tree(refspec)).transpose()?;

        let mut diff = self
            .inner
            .diff_tree_to_tree(base_tree.as_ref(), Some(&head_tree), None)?;

        let mut find_opts = git2::DiffFindOptions::new();
        find_opts.renames(true);
        find_opts.copies(true);
        find_opts.copies_from_unmodified(true);
        diff.find_similar(Some(&mut find_opts))?;

        let mut changes = Vec::new();

        for delta in diff.deltas() {
            let status = match delta.status() {
                git2::Delta::Added => FileStatus::Added,
                git2::Delta::Deleted => FileStatus::Deleted,
                git2::Delta::Modified => FileStatus::Modified,
                git2::Delta::Renamed => FileStatus::Renamed,
                git2::Delta::Copied => FileStatus::Copied,
                _ => continue,
            };

            let path = delta
                .new_file()
                .path()
                .or_else(|| delta.old_file().path())
                .map(PathBuf::from)
                .ok_or(GitError::MissingDeltaPath)?;

            let mut change = FileChange::new(path, status);

            if status == FileStatus::Renamed || status == FileStatus::Copied {
                if let Some(old_path) = delta.old_file().path() {
                    change = change.with_old_path(old_path.to_path_buf());
                }
            }

            changes.push(change);
        }

        Ok(changes)
    }

    /// # Errors
    ///
    /// Returns [`GitError::RefNotFound`] if the base reference cannot be resolved.
    pub fn changed_files_from_head(&self, base: &str) -> Result<Vec<FileChange>> {
        self.changed_files(Some(base), "HEAD")
    }

    fn resolve_tree(&self, refspec: &str) -> Result<git2::Tree<'_>> {
        let obj = self
            .inner
            .revparse_single(refspec)
            .or_else(|original_err| self.try_remote_tracking_ref(refspec).ok_or(original_err))
            .map_err(|source| GitError::RefNotFound {
                refspec: refspec.to_string(),
                source,
            })?;

        obj.peel_to_tree().map_err(|source| GitError::NotATree {
            refspec: refspec.to_string(),
            source,
        })
    }

    fn try_remote_tracking_ref(&self, refspec: &str) -> Option<git2::Object<'_>> {
        if !refspec.starts_with("refs/") && refspec.contains('/') {
            self.inner
                .revparse_single(&format!("refs/remotes/{refspec}"))
                .ok()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::setup_test_repo;
    use crate::{FileChange, FileStatus, GitError};
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn detect_added_file() -> anyhow::Result<()> {
        let (dir, repo) = setup_test_repo()?;

        fs::write(dir.path().join("new_file.txt"), "content")?;

        let mut index = repo.inner.index()?;
        index.add_path(std::path::Path::new("new_file.txt"))?;
        index.write()?;

        let sig = git2::Signature::now("Test", "test@example.com")?;
        let tree_id = index.write_tree()?;
        let tree = repo.inner.find_tree(tree_id)?;
        let parent = repo.inner.head()?.peel_to_commit()?;
        repo.inner
            .commit(Some("HEAD"), &sig, &sig, "Add file", &tree, &[&parent])?;

        let changes = repo.changed_files_from_head("HEAD~1")?;
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].status, FileStatus::Added);
        assert_eq!(changes[0].path.to_string_lossy(), "new_file.txt");

        Ok(())
    }

    #[test]
    fn detect_modified_file() -> anyhow::Result<()> {
        let (dir, repo) = setup_test_repo()?;

        fs::write(dir.path().join("file.txt"), "initial")?;
        let mut index = repo.inner.index()?;
        index.add_path(std::path::Path::new("file.txt"))?;
        index.write()?;

        let sig = git2::Signature::now("Test", "test@example.com")?;
        let tree_id = index.write_tree()?;
        let tree = repo.inner.find_tree(tree_id)?;
        let parent = repo.inner.head()?.peel_to_commit()?;
        repo.inner
            .commit(Some("HEAD"), &sig, &sig, "Add file", &tree, &[&parent])?;

        fs::write(dir.path().join("file.txt"), "modified")?;
        let mut index = repo.inner.index()?;
        index.add_path(std::path::Path::new("file.txt"))?;
        index.write()?;

        let tree_id = index.write_tree()?;
        let tree = repo.inner.find_tree(tree_id)?;
        let parent = repo.inner.head()?.peel_to_commit()?;
        repo.inner
            .commit(Some("HEAD"), &sig, &sig, "Modify file", &tree, &[&parent])?;

        let changes = repo.changed_files_from_head("HEAD~1")?;
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].status, FileStatus::Modified);

        Ok(())
    }

    #[test]
    fn detect_deleted_file() -> anyhow::Result<()> {
        let (dir, repo) = setup_test_repo()?;

        fs::write(dir.path().join("file.txt"), "content")?;
        let mut index = repo.inner.index()?;
        index.add_path(std::path::Path::new("file.txt"))?;
        index.write()?;

        let sig = git2::Signature::now("Test", "test@example.com")?;
        let tree_id = index.write_tree()?;
        let tree = repo.inner.find_tree(tree_id)?;
        let parent = repo.inner.head()?.peel_to_commit()?;
        repo.inner
            .commit(Some("HEAD"), &sig, &sig, "Add file", &tree, &[&parent])?;

        fs::remove_file(dir.path().join("file.txt"))?;
        let mut index = repo.inner.index()?;
        index.remove_path(std::path::Path::new("file.txt"))?;
        index.write()?;

        let tree_id = index.write_tree()?;
        let tree = repo.inner.find_tree(tree_id)?;
        let parent = repo.inner.head()?.peel_to_commit()?;
        repo.inner
            .commit(Some("HEAD"), &sig, &sig, "Delete file", &tree, &[&parent])?;

        let changes = repo.changed_files_from_head("HEAD~1")?;
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].status, FileStatus::Deleted);

        Ok(())
    }

    #[test]
    fn ref_not_found_error() -> anyhow::Result<()> {
        let (_dir, repo) = setup_test_repo()?;

        let result = repo.changed_files_from_head("nonexistent-ref");
        assert!(matches!(result, Err(GitError::RefNotFound { .. })));

        Ok(())
    }

    #[test]
    fn resolve_remote_tracking_ref_shorthand() -> anyhow::Result<()> {
        let (dir, repo) = setup_test_repo()?;

        let base_commit_oid = repo.inner.head()?.peel_to_commit()?.id();

        fs::write(dir.path().join("feature.txt"), "content")?;
        let mut index = repo.inner.index()?;
        index.add_path(std::path::Path::new("feature.txt"))?;
        index.write()?;

        let sig = git2::Signature::now("Test", "test@example.com")?;
        let tree_id = index.write_tree()?;
        let tree = repo.inner.find_tree(tree_id)?;
        let parent = repo.inner.head()?.peel_to_commit()?;
        repo.inner
            .commit(Some("HEAD"), &sig, &sig, "Add feature", &tree, &[&parent])?;

        repo.inner
            .reference("refs/remotes/origin/main", base_commit_oid, false, "")?;

        let changes = repo.changed_files_from_head("origin/main")?;

        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].status, FileStatus::Added);
        assert_eq!(changes[0].path.to_string_lossy(), "feature.txt");

        Ok(())
    }

    #[test]
    fn remote_tracking_ref_not_found_returns_error() -> anyhow::Result<()> {
        let (_dir, repo) = setup_test_repo()?;

        let result = repo.changed_files_from_head("origin/nonexistent");
        assert!(matches!(result, Err(GitError::RefNotFound { .. })));

        Ok(())
    }

    #[test]
    fn detect_renamed_file() -> anyhow::Result<()> {
        let (dir, repo) = setup_test_repo()?;
        let path = std::path::Path::new;

        fs::write(dir.path().join("original.txt"), "content")?;
        let mut index = repo.inner.index()?;
        index.add_path(path("original.txt"))?;
        index.write()?;

        let sig = git2::Signature::now("Test", "test@example.com")?;
        let tree_id = index.write_tree()?;
        let tree = repo.inner.find_tree(tree_id)?;
        let parent = repo.inner.head()?.peel_to_commit()?;
        repo.inner
            .commit(Some("HEAD"), &sig, &sig, "Add file", &tree, &[&parent])?;

        fs::rename(
            dir.path().join("original.txt"),
            dir.path().join("renamed.txt"),
        )?;
        let mut index = repo.inner.index()?;
        index.remove_path(path("original.txt"))?;
        index.add_path(path("renamed.txt"))?;
        index.write()?;

        let tree_id = index.write_tree()?;
        let tree = repo.inner.find_tree(tree_id)?;
        let parent = repo.inner.head()?.peel_to_commit()?;
        repo.inner
            .commit(Some("HEAD"), &sig, &sig, "Rename file", &tree, &[&parent])?;

        let changes = repo.changed_files_from_head("HEAD~1")?;
        assert_eq!(changes.len(), 1);

        let rename = &changes[0];
        assert_eq!(rename.status, FileStatus::Renamed);
        assert_eq!(rename.path, PathBuf::from("renamed.txt"));
        assert_eq!(rename.old_path, Some(PathBuf::from("original.txt")));

        Ok(())
    }

    #[test]
    fn new_file_alongside_existing_is_detected_as_added() -> anyhow::Result<()> {
        let (dir, repo) = setup_test_repo()?;
        let path = std::path::Path::new;

        let content = "This is a longer piece of content.";
        fs::write(dir.path().join("original.txt"), content)?;
        let mut index = repo.inner.index()?;
        index.add_path(path("original.txt"))?;
        index.write()?;

        let sig = git2::Signature::now("Test", "test@example.com")?;
        let tree_id = index.write_tree()?;
        let tree = repo.inner.find_tree(tree_id)?;
        let parent = repo.inner.head()?.peel_to_commit()?;
        repo.inner
            .commit(Some("HEAD"), &sig, &sig, "Add file", &tree, &[&parent])?;

        fs::copy(dir.path().join("original.txt"), dir.path().join("copy.txt"))?;
        let mut index = repo.inner.index()?;
        index.add_path(path("copy.txt"))?;
        index.write()?;

        let tree_id = index.write_tree()?;
        let tree = repo.inner.find_tree(tree_id)?;
        let parent = repo.inner.head()?.peel_to_commit()?;
        repo.inner
            .commit(Some("HEAD"), &sig, &sig, "Copy file", &tree, &[&parent])?;

        let changes = repo.changed_files_from_head("HEAD~1")?;
        assert_eq!(changes.len(), 1);

        let change = &changes[0];
        assert!(
            change.status == FileStatus::Added || change.status == FileStatus::Copied,
            "new file should be detected as Added or Copied, got {:?}",
            change.status
        );
        assert_eq!(change.path, PathBuf::from("copy.txt"));

        Ok(())
    }

    #[test]
    fn none_base_shows_all_files_as_added() -> anyhow::Result<()> {
        let (dir, repo) = setup_test_repo()?;

        fs::write(dir.path().join("a.txt"), "alpha")?;
        fs::write(dir.path().join("b.txt"), "beta")?;

        let mut index = repo.inner.index()?;
        index.add_path(std::path::Path::new("a.txt"))?;
        index.add_path(std::path::Path::new("b.txt"))?;
        index.write()?;

        let sig = git2::Signature::now("Test", "test@example.com")?;
        let tree_id = index.write_tree()?;
        let tree = repo.inner.find_tree(tree_id)?;
        let parent = repo.inner.head()?.peel_to_commit()?;
        repo.inner
            .commit(Some("HEAD"), &sig, &sig, "Add files", &tree, &[&parent])?;

        let changes = repo.changed_files(None, "HEAD")?;

        assert!(
            changes.len() >= 2,
            "expected at least 2 files, got {}",
            changes.len()
        );
        assert!(
            changes.iter().all(|c| c.status == FileStatus::Added),
            "all files should be Added when diffing against empty tree"
        );

        let paths: Vec<_> = changes.iter().map(|c| c.path.clone()).collect();
        assert!(paths.contains(&PathBuf::from("a.txt")));
        assert!(paths.contains(&PathBuf::from("b.txt")));

        Ok(())
    }

    #[test]
    fn blob_ref_returns_not_a_tree_error() -> anyhow::Result<()> {
        let (_dir, repo) = setup_test_repo()?;

        let blob_oid = repo.inner.blob(b"not a tree")?;
        repo.inner
            .reference("refs/heads/blob-ref", blob_oid, false, "")?;

        let result = repo.changed_files_from_head("refs/heads/blob-ref");
        assert!(matches!(result, Err(GitError::NotATree { .. })));

        Ok(())
    }

    #[test]
    fn heterogeneous_diff_detects_added_modified_and_deleted() -> anyhow::Result<()> {
        let (dir, repo) = setup_test_repo()?;
        let sig = git2::Signature::now("Test", "test@example.com")?;

        fs::write(dir.path().join("to_modify.txt"), "original")?;
        fs::write(dir.path().join("to_delete.txt"), "doomed")?;

        let mut index = repo.inner.index()?;
        index.add_path(std::path::Path::new("to_modify.txt"))?;
        index.add_path(std::path::Path::new("to_delete.txt"))?;
        index.write()?;

        let tree_id = index.write_tree()?;
        let tree = repo.inner.find_tree(tree_id)?;
        let parent = repo.inner.head()?.peel_to_commit()?;
        repo.inner
            .commit(Some("HEAD"), &sig, &sig, "Setup files", &tree, &[&parent])?;

        fs::write(dir.path().join("to_modify.txt"), "changed")?;
        fs::remove_file(dir.path().join("to_delete.txt"))?;
        fs::write(dir.path().join("brand_new.txt"), "hello")?;

        let mut index = repo.inner.index()?;
        index.add_path(std::path::Path::new("to_modify.txt"))?;
        index.remove_path(std::path::Path::new("to_delete.txt"))?;
        index.add_path(std::path::Path::new("brand_new.txt"))?;
        index.write()?;

        let tree_id = index.write_tree()?;
        let tree = repo.inner.find_tree(tree_id)?;
        let parent = repo.inner.head()?.peel_to_commit()?;
        repo.inner.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "Add, modify, delete",
            &tree,
            &[&parent],
        )?;

        let changes = repo.changed_files_from_head("HEAD~1")?;

        let statuses: std::collections::HashMap<_, _> = changes
            .iter()
            .map(|c| (c.path.to_string_lossy().into_owned(), c.status))
            .collect();

        assert_eq!(statuses.len(), 3);
        assert_eq!(statuses["brand_new.txt"], FileStatus::Added);
        assert_eq!(statuses["to_modify.txt"], FileStatus::Modified);
        assert_eq!(statuses["to_delete.txt"], FileStatus::Deleted);

        Ok(())
    }

    #[test]
    fn diff_between_two_explicit_non_head_refs() -> anyhow::Result<()> {
        let (dir, repo) = setup_test_repo()?;
        let sig = git2::Signature::now("Test", "test@example.com")?;

        let base_oid = repo.inner.head()?.peel_to_commit()?.id();

        fs::write(dir.path().join("first.txt"), "one")?;
        let mut index = repo.inner.index()?;
        index.add_path(std::path::Path::new("first.txt"))?;
        index.write()?;
        let tree_id = index.write_tree()?;
        let tree = repo.inner.find_tree(tree_id)?;
        let parent = repo.inner.find_commit(base_oid)?;
        let mid_oid =
            repo.inner
                .commit(Some("HEAD"), &sig, &sig, "First commit", &tree, &[&parent])?;

        fs::write(dir.path().join("second.txt"), "two")?;
        let mut index = repo.inner.index()?;
        index.add_path(std::path::Path::new("second.txt"))?;
        index.write()?;
        let tree_id = index.write_tree()?;
        let tree = repo.inner.find_tree(tree_id)?;
        let parent = repo.inner.find_commit(mid_oid)?;
        let tip_oid =
            repo.inner
                .commit(Some("HEAD"), &sig, &sig, "Second commit", &tree, &[&parent])?;

        let base_hex = mid_oid.to_string();
        let tip_hex = tip_oid.to_string();

        let changes = repo.changed_files(Some(&base_hex), &tip_hex)?;

        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].status, FileStatus::Added);
        assert_eq!(changes[0].path, PathBuf::from("second.txt"));

        Ok(())
    }

    #[test]
    fn file_change_with_old_path() {
        let change = FileChange::new(PathBuf::from("new.txt"), FileStatus::Renamed)
            .with_old_path(PathBuf::from("old.txt"));

        assert_eq!(change.path, PathBuf::from("new.txt"));
        assert_eq!(change.status, FileStatus::Renamed);
        assert_eq!(change.old_path, Some(PathBuf::from("old.txt")));
    }
}
