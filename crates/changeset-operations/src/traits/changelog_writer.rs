use std::path::{Path, PathBuf};

use changeset_changelog::{RepositoryInfo, VersionRelease};

use crate::Result;

#[derive(Debug, Clone)]
pub struct ChangelogWriteResult {
    pub path: PathBuf,
    pub created: bool,
}

pub trait ChangelogWriter: Send + Sync {
    /// # Errors
    ///
    /// Returns an error if the changelog cannot be read or written.
    fn write_release(
        &self,
        changelog_path: &Path,
        release: &VersionRelease,
        repo_info: Option<&RepositoryInfo>,
        previous_version: Option<&str>,
    ) -> Result<ChangelogWriteResult>;

    fn changelog_exists(&self, path: &Path) -> bool;

    /// # Errors
    ///
    /// Returns an error if the changelog cannot be restored.
    fn restore_changelog(&self, path: &Path, content: &str) -> Result<()>;

    /// # Errors
    ///
    /// Returns an error if the changelog cannot be deleted.
    fn delete_changelog(&self, path: &Path) -> Result<()>;
}
