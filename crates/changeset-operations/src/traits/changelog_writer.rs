use std::path::{Path, PathBuf};

use changeset_changelog::{RepositoryInfo, VersionRelease};
use gset::Getset;

use crate::Result;

#[derive(Debug, Clone, Getset)]
pub struct ChangelogWriteResult {
    #[getset(get, vis = "pub")]
    path: PathBuf,
    #[getset(get_copy, vis = "pub")]
    created: bool,
}

impl ChangelogWriteResult {
    pub fn new(path: PathBuf, created: bool) -> Self {
        Self { path, created }
    }
}

pub trait ChangelogWriter: Send + Sync {
    /// # Errors
    ///
    /// Propagates changelog read/write errors.
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
    /// Propagates write errors.
    fn restore_changelog(&self, path: &Path, content: &str) -> Result<()>;

    /// # Errors
    ///
    /// Propagates deletion errors.
    fn delete_changelog(&self, path: &Path) -> Result<()>;
}
