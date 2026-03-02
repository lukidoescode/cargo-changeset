use std::path::Path;

use changeset_manifest::{InitConfig, MetadataSection};
use semver::Version;

use crate::Result;

pub trait ManifestVersionWriter: Send + Sync {
    /// # Errors
    ///
    /// Returns an error if the manifest cannot be read or written.
    fn write_version(&self, manifest_path: &Path, new_version: &Version) -> Result<()>;

    /// # Errors
    ///
    /// Returns an error if the version does not match the expected value.
    fn verify_version(&self, manifest_path: &Path, expected: &Version) -> Result<()>;
}

pub trait ManifestDependencyWriter: Send + Sync {
    /// Updates the version constraint for a dependency in all relevant sections
    /// of a Cargo.toml file.
    ///
    /// # Errors
    ///
    /// Returns an error if the manifest cannot be read, parsed, or written.
    fn update_dependency_version(
        &self,
        manifest_path: &Path,
        dependency_name: &str,
        new_version: &Version,
    ) -> Result<bool>;
}

pub trait WorkspaceVersionManager: Send + Sync {
    /// Reads the workspace package version from a root manifest.
    ///
    /// Returns `Ok(None)` if the workspace version field is not present.
    ///
    /// # Errors
    ///
    /// Returns an error if the manifest cannot be read or parsed.
    fn read_workspace_version(&self, manifest_path: &Path) -> Result<Option<Version>>;

    /// # Errors
    ///
    /// Returns an error if the manifest cannot be read or written.
    fn remove_workspace_version(&self, manifest_path: &Path) -> Result<()>;

    /// Writes or restores the workspace package version in a root manifest.
    ///
    /// # Errors
    ///
    /// Returns an error if the manifest cannot be read, parsed, or written.
    fn write_workspace_version(&self, manifest_path: &Path, version: &Version) -> Result<()>;
}

pub trait ManifestMetadataWriter: Send + Sync {
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
