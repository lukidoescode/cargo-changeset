use std::path::{Path, PathBuf};

use changeset_core::Changeset;

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
}

pub trait ChangesetWriter: Send + Sync {
    /// # Errors
    ///
    /// Returns an error if the changeset cannot be serialized or written.
    fn write_changeset(&self, changeset_dir: &Path, changeset: &Changeset) -> Result<String>;

    fn filename_exists(&self, changeset_dir: &Path, filename: &str) -> bool;
}
