use std::path::Path;

use changeset_core::ManifestFormat;
use changeset_manifest::{InitConfig, MetadataSection};
use semver::Version;

use crate::Result;
use crate::traits::InheritedVersionChecker;

pub trait FullManifestWriter:
    ManifestVersionWriter
    + ManifestDependencyWriter
    + WorkspaceVersionManager
    + InheritedVersionChecker
    + LockfileUpdater
{
}
impl<
    T: ManifestVersionWriter
        + ManifestDependencyWriter
        + WorkspaceVersionManager
        + InheritedVersionChecker
        + LockfileUpdater,
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

pub trait LockfileUpdater: Send + Sync {
    /// # Errors
    ///
    /// Returns an error if the `cargo update --workspace` command fails.
    fn update_lockfile(&self, project_root: &Path) -> Result<()>;

    /// # Errors
    ///
    /// Returns an error if the lockfile exists but cannot be read.
    fn read_lockfile(&self, project_root: &Path) -> Result<Option<Vec<u8>>>;

    /// # Errors
    ///
    /// Returns an error if the lockfile cannot be written.
    fn restore_lockfile(&self, project_root: &Path, content: &[u8]) -> Result<()>;

    /// # Errors
    ///
    /// Returns an error if the lockfile exists but cannot be removed.
    fn remove_lockfile(&self, project_root: &Path) -> Result<()>;
}

pub trait ExternalManifestVersionWriter: Send + Sync {
    /// # Errors
    ///
    /// Propagates external manifest write errors.
    fn write_external_version(
        &self,
        manifest_path: &Path,
        format: ManifestFormat,
        version_field_path: &str,
        new_version: &Version,
    ) -> Result<()>;

    /// # Errors
    ///
    /// Propagates external manifest read/verification errors.
    fn verify_external_version(
        &self,
        manifest_path: &Path,
        format: ManifestFormat,
        version_field_path: &str,
        expected: &Version,
    ) -> Result<()>;

    /// # Errors
    ///
    /// Propagates external manifest write errors.
    fn restore_external_version(
        &self,
        manifest_path: &Path,
        format: ManifestFormat,
        version_field_path: &str,
        version_str: &str,
    ) -> Result<()>;
}

pub trait ExternalManifestVersionReader: Send + Sync {
    /// # Errors
    ///
    /// Propagates external manifest read errors.
    fn read_external_version(
        &self,
        manifest_path: &Path,
        format: ManifestFormat,
        version_field_path: &str,
    ) -> Result<String>;
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
