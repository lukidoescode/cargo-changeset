use std::path::{Path, PathBuf};

use changeset_core::Changeset;
use semver::Version;

use crate::Result;

pub trait ChangesetReader: Send + Sync {
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    fn read_changeset(&self, path: &Path) -> Result<Changeset>;

    /// # Errors
    ///
    /// Returns an error if the directory cannot be read.
    fn list_changesets(&self, changeset_dir: &Path) -> Result<Vec<PathBuf>>;

    /// # Errors
    ///
    /// Returns an error if the directory cannot be read.
    fn list_consumed_changesets(&self, changeset_dir: &Path) -> Result<Vec<PathBuf>>;
}

pub trait ChangesetWriter: Send + Sync {
    /// # Errors
    ///
    /// Returns an error if the changeset cannot be serialized or written.
    fn write_changeset(&self, changeset_dir: &Path, changeset: &Changeset) -> Result<String>;

    fn filename_exists(&self, changeset_dir: &Path, filename: &str) -> bool;

    /// # Errors
    ///
    /// Returns an error if changesets cannot be read, parsed, or written.
    fn mark_consumed_for_prerelease(
        &self,
        changeset_dir: &Path,
        paths: &[&Path],
        version: &Version,
    ) -> Result<()>;

    /// # Errors
    ///
    /// Returns an error if changesets cannot be read, parsed, or written.
    fn clear_consumed_for_prerelease(&self, changeset_dir: &Path, paths: &[&Path]) -> Result<()>;
}
