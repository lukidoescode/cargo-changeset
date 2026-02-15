use std::path::Path;

use changeset_project::{GraduationState, PrereleaseState};

use crate::Result;

/// Reads and writes release state configuration files.
///
/// This trait handles persistence of release management state:
/// - `pre-release.toml`: Maps crate names to prerelease tags
/// - `graduation.toml`: Lists crates queued for 0.x -> 1.0.0 graduation
pub trait ReleaseStateIO: Send + Sync {
    /// Loads prerelease state from `.changeset/pre-release.toml`.
    /// Returns `Ok(None)` if the file doesn't exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the file exists but cannot be read or parsed.
    fn load_prerelease_state(&self, changeset_dir: &Path) -> Result<Option<PrereleaseState>>;

    /// Saves prerelease state to `.changeset/pre-release.toml`.
    /// Deletes the file if state is empty.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written or deleted.
    fn save_prerelease_state(&self, changeset_dir: &Path, state: &PrereleaseState) -> Result<()>;

    /// Loads graduation state from `.changeset/graduation.toml`.
    /// Returns `Ok(None)` if the file doesn't exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the file exists but cannot be read or parsed.
    fn load_graduation_state(&self, changeset_dir: &Path) -> Result<Option<GraduationState>>;

    /// Saves graduation state to `.changeset/graduation.toml`.
    /// Deletes the file if state is empty.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written or deleted.
    fn save_graduation_state(&self, changeset_dir: &Path, state: &GraduationState) -> Result<()>;
}
