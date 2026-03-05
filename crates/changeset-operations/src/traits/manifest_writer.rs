use std::path::Path;

use changeset_manifest::{InitConfig, MetadataSection};
use semver::Version;

use crate::Result;
use crate::traits::InheritedVersionChecker;

pub trait FullManifestWriter:
    ManifestVersionWriter + ManifestDependencyWriter + WorkspaceVersionManager + InheritedVersionChecker
{
}
impl<
    T: ManifestVersionWriter
        + ManifestDependencyWriter
        + WorkspaceVersionManager
        + InheritedVersionChecker,
> FullManifestWriter for T
{
}

pub trait ManifestVersionWriter: Send + Sync {
    /// # Errors
    ///
    /// Propagates manifest write errors.
    fn write_version(&self, manifest_path: &Path, new_version: &Version) -> Result<()>;

    /// # Errors
    ///
    /// Propagates manifest read/verification errors.
    fn verify_version(&self, manifest_path: &Path, expected: &Version) -> Result<()>;
}

pub trait ManifestDependencyWriter: Send + Sync {
    /// # Errors
    ///
    /// Propagates manifest read/write errors.
    fn update_dependency_version(
        &self,
        manifest_path: &Path,
        dependency_name: &str,
        new_version: &Version,
    ) -> Result<bool>;
}

pub trait WorkspaceVersionManager: Send + Sync {
    /// Returns `Ok(None)` if the workspace version field is not present.
    ///
    /// # Errors
    ///
    /// Propagates manifest read errors.
    fn read_workspace_version(&self, manifest_path: &Path) -> Result<Option<Version>>;

    /// # Errors
    ///
    /// Propagates manifest write errors.
    fn remove_workspace_version(&self, manifest_path: &Path) -> Result<()>;

    /// # Errors
    ///
    /// Propagates manifest write errors.
    fn write_workspace_version(&self, manifest_path: &Path, version: &Version) -> Result<()>;
}

pub trait ManifestMetadataWriter: Send + Sync {
    /// # Errors
    ///
    /// Propagates manifest write errors.
    fn write_metadata(
        &self,
        manifest_path: &Path,
        section: MetadataSection,
        config: &InitConfig,
    ) -> Result<()>;
}
