//! Changeset I/O traits for reading and writing changeset files.
//!
//! # Consumed Changeset Lifecycle
//!
//! Changesets follow a specific lifecycle during the prerelease workflow:
//!
//! 1. **Creation**: A changeset file is created via `cargo changeset add` with no
//!    `consumedForPrerelease` field.
//!
//! 2. **Consumption**: When a prerelease is created (`cargo changeset release --prerelease`),
//!    changesets are marked as consumed by setting `consumedForPrerelease` to the prerelease
//!    version string (e.g., "1.0.1-alpha.1"). This prevents the same changes from being
//!    included in subsequent prereleases while preserving the changeset for the eventual
//!    stable release.
//!
//! 3. **Exclusion**: Consumed changesets are excluded from `list_changesets()` but included
//!    in `list_consumed_changesets()`. This ensures subsequent prereleases only process
//!    new changes.
//!
//! 4. **Aggregation**: When graduating from prerelease to stable, consumed changesets are
//!    loaded and aggregated into the final changelog entry alongside any new changesets.
//!    The `consumedForPrerelease` flag is cleared during graduation.
//!
//! 5. **Deletion**: After a stable release, all changeset files (both previously consumed
//!    and newly processed) are deleted, completing the lifecycle.

use std::path::{Path, PathBuf};

use changeset_core::Changeset;
use semver::Version;

use crate::Result;

/// Reads changeset files from the filesystem.
///
/// See the [module-level documentation](self) for details on the consumed changeset lifecycle.
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

    #[must_use]
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
