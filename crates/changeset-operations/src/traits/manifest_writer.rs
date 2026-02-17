use std::path::Path;

use changeset_manifest::{InitConfig, MetadataSection};
use semver::Version;

use super::inherited_version_checker::InheritedVersionChecker;
use crate::Result;

pub trait ManifestWriter: InheritedVersionChecker + Send + Sync {
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

    /// Writes changeset configuration to the metadata section of a Cargo.toml file.
    ///
    /// # Errors
    ///
    /// Returns an error if the manifest cannot be read, parsed, or written.
    fn write_metadata(
        &self,
        manifest_path: &Path,
        section: MetadataSection,
        config: &InitConfig,
    ) -> Result<()>;
}
