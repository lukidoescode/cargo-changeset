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
}
