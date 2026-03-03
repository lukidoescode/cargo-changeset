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
    /// Returns an error if the repository cannot be opened or the diff fails.
    fn changed_files(&self, project_root: &Path, base: &str, head: &str)
    -> Result<Vec<FileChange>>;
}

pub trait GitStatusProvider: Send + Sync {
    /// # Errors
    ///
    /// Returns an error if the repository cannot be opened.
    fn is_working_tree_clean(&self, project_root: &Path) -> Result<bool>;

    /// # Errors
    ///
    /// Returns an error if the repository cannot be opened.
    fn current_branch(&self, project_root: &Path) -> Result<String>;

    /// # Errors
    ///
    /// Returns an error if the repository cannot be opened.
    fn remote_url(&self, project_root: &Path) -> Result<Option<String>>;
}

pub trait GitStagingProvider: Send + Sync {
    /// # Errors
    ///
    /// Returns an error if any file cannot be staged.
    fn stage_files(&self, project_root: &Path, paths: &[&Path]) -> Result<()>;

    /// Deletes files from the filesystem and stages the deletions in git.
    ///
    /// This is a fail-fast operation: if any file does not exist or cannot be deleted,
    /// an error is returned immediately and no further files are processed.
    ///
    /// # Errors
    ///
    /// Returns an error if any file cannot be deleted or staged.
    fn delete_files(&self, project_root: &Path, paths: &[&Path]) -> Result<()>;
}

pub trait GitCommitProvider: Send + Sync {
    /// # Errors
    ///
    /// Returns an error if the commit cannot be created.
    fn commit(&self, project_root: &Path, message: &str) -> Result<CommitInfo>;

    /// Performs a soft reset to the parent of HEAD (HEAD~1),
    /// undoing the last commit while keeping changes staged.
    ///
    /// # Errors
    ///
    /// Returns an error if HEAD has no parent or the reset fails.
    fn reset_to_parent(&self, project_root: &Path) -> Result<()>;
}

pub trait GitTagProvider: Send + Sync {
    /// # Errors
    ///
    /// Returns an error if the tag already exists or cannot be created.
    fn create_tag(&self, project_root: &Path, tag_name: &str, message: &str) -> Result<TagInfo>;

    /// Returns `Ok(true)` if the tag was deleted, `Ok(false)` if the tag was not found.
    ///
    /// # Errors
    ///
    /// Returns an error for failures other than "tag not found".
    fn delete_tag(&self, project_root: &Path, tag_name: &str) -> Result<bool>;
}
