use std::path::Path;

use changeset_git::{CommitInfo, FileChange, TagInfo};

use crate::Result;

pub trait GitProvider: Send + Sync {
    /// # Errors
    ///
    /// Returns an error if the repository cannot be opened or diff fails.
    fn changed_files(&self, project_root: &Path, base: &str, head: &str)
    -> Result<Vec<FileChange>>;

    /// # Errors
    ///
    /// Returns an error if the repository cannot be opened or status check fails.
    fn is_working_tree_clean(&self, project_root: &Path) -> Result<bool>;

    /// # Errors
    ///
    /// Returns an error if the repository cannot be opened or HEAD is detached.
    fn current_branch(&self, project_root: &Path) -> Result<String>;

    /// # Errors
    ///
    /// Returns an error if staging any of the files fails.
    fn stage_files(&self, project_root: &Path, paths: &[&Path]) -> Result<()>;

    /// # Errors
    ///
    /// Returns an error if the commit cannot be created.
    fn commit(&self, project_root: &Path, message: &str) -> Result<CommitInfo>;

    /// # Errors
    ///
    /// Returns an error if the tag cannot be created or already exists.
    fn create_tag(&self, project_root: &Path, tag_name: &str, message: &str) -> Result<TagInfo>;

    /// # Errors
    ///
    /// Returns an error if the repository cannot be opened.
    fn remote_url(&self, project_root: &Path) -> Result<Option<String>>;

    /// Deletes files from the filesystem and stages the deletions in git.
    ///
    /// This is a fail-fast operation: if any file does not exist or cannot be deleted,
    /// an error is returned immediately and no further files are processed.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Any file in `paths` does not exist
    /// - Any file cannot be deleted (permissions, in use, etc.)
    /// - The git index cannot be updated to stage the deletion
    fn delete_files(&self, project_root: &Path, paths: &[&Path]) -> Result<()>;

    /// Deletes a tag by name.
    ///
    /// Returns `Ok(true)` if the tag was deleted, `Ok(false)` if the tag was not found.
    ///
    /// # Errors
    ///
    /// Returns an error if the delete operation fails for reasons other than "not found".
    fn delete_tag(&self, project_root: &Path, tag_name: &str) -> Result<bool>;

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
    fn reset_to_parent(&self, project_root: &Path) -> Result<()>;
}
