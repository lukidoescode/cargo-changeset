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
    /// Propagates repository or diff errors.
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
    /// Propagates staging errors.
    fn stage_files(&self, project_root: &Path, paths: &[&Path]) -> Result<()>;

    /// Deletes files from the filesystem and stages the deletions in git.
    ///
    /// This is a fail-fast operation: if any file does not exist or cannot be deleted,
    /// an error is returned immediately and no further files are processed.
    ///
    /// # Errors
    ///
    /// Propagates filesystem or staging errors.
    fn delete_files(&self, project_root: &Path, paths: &[&Path]) -> Result<()>;
}

pub trait GitCommitProvider: Send + Sync {
    /// # Errors
    ///
    /// Propagates commit errors.
    fn commit(&self, project_root: &Path, message: &str) -> Result<CommitInfo>;

    /// Performs a soft reset to the parent of HEAD (HEAD~1).
    ///
    /// This undoes the last commit while keeping changes staged.
    ///
    /// # Errors
    ///
    /// Propagates reset errors, including when HEAD has no parent.
    fn reset_to_parent(&self, project_root: &Path) -> Result<()>;
}

pub trait GitTagProvider: Send + Sync {
    /// # Errors
    ///
    /// Propagates tag creation errors, including when the tag already exists.
    fn create_tag(&self, project_root: &Path, tag_name: &str, message: &str) -> Result<TagInfo>;

    /// Returns `Ok(true)` if the tag was deleted, `Ok(false)` if the tag was not found.
    ///
    /// # Errors
    ///
    /// Propagates errors other than "tag not found".
    fn delete_tag(&self, project_root: &Path, tag_name: &str) -> Result<bool>;
}
