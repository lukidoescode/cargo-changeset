use std::path::Path;

use semver::Version;

use crate::Result;

pub trait ManifestWriter: Send + Sync {
    /// # Errors
    ///
    /// Returns an error if the manifest cannot be read or written.
    fn write_version(&self, manifest_path: &Path, new_version: &Version) -> Result<()>;

    /// # Errors
    ///
    /// Returns an error if the manifest cannot be read or written.
    fn remove_workspace_version(&self, manifest_path: &Path) -> Result<()>;

    /// # Errors
    ///
    /// Returns an error if the version does not match the expected value.
    fn verify_version(&self, manifest_path: &Path, expected: &Version) -> Result<()>;

    /// # Errors
    ///
    /// Returns an error if the manifest cannot be read.
    fn has_inherited_version(&self, manifest_path: &Path) -> Result<bool>;
}
