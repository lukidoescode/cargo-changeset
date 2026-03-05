use std::path::Path;

use changeset_git::{CommitInfo, FileChange, TagInfo};

use crate::Result;

pub trait FullGitProvider:
    GitDiffProvider + GitStatusProvider + GitStagingProvider + GitCommitProvider + GitTagProvider
{
}
impl<
    T: GitDiffProvider + GitStatusProvider + GitStagingProvider + GitCommitProvider + GitTagProvider,
> FullGitProvider for T
{
}

pub trait GitDiffProvider: Send + Sync {
    /// # Errors
    ///
    /// Propagates repository errors.
    fn changed_files(&self, project_root: &Path, base: &str, head: &str)
    -> Result<Vec<FileChange>>;
}

pub trait GitStatusProvider: Send + Sync {
    /// # Errors
    ///
    /// Propagates repository errors.
    fn is_working_tree_clean(&self, project_root: &Path) -> Result<bool>;

    /// # Errors
    ///
    /// Propagates repository errors.
    fn current_branch(&self, project_root: &Path) -> Result<String>;

    /// # Errors
    ///
    /// Propagates repository errors.
    fn remote_url(&self, project_root: &Path) -> Result<Option<String>>;
}

pub trait GitStagingProvider: Send + Sync {
    /// # Errors
    ///
    /// Propagates repository errors.
    fn stage_files(&self, project_root: &Path, paths: &[&Path]) -> Result<()>;

    /// Fail-fast: if any file does not exist or cannot be deleted,
    /// returns an error immediately and no further files are processed.
    ///
    /// # Errors
    ///
    /// Propagates repository errors.
    fn delete_files(&self, project_root: &Path, paths: &[&Path]) -> Result<()>;
}

pub trait GitCommitProvider: Send + Sync {
    /// # Errors
    ///
    /// Propagates repository errors.
    fn commit(&self, project_root: &Path, message: &str) -> Result<CommitInfo>;

    /// Soft reset to HEAD~1, undoing the last commit while keeping changes staged.
    ///
    /// # Errors
    ///
    /// Propagates repository errors.
    fn reset_to_parent(&self, project_root: &Path) -> Result<()>;
}

pub trait GitTagProvider: Send + Sync {
    /// # Errors
    ///
    /// Propagates repository errors.
    fn create_tag(&self, project_root: &Path, tag_name: &str, message: &str) -> Result<TagInfo>;

    /// Returns `Ok(true)` if deleted, `Ok(false)` if not found.
    ///
    /// # Errors
    ///
    /// Propagates repository errors.
    fn delete_tag(&self, project_root: &Path, tag_name: &str) -> Result<bool>;
}
