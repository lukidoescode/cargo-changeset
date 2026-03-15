use std::path::Path;

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
    /// Returns an error if the `cargo generate-lockfile` command fails.
    fn generate_lockfile(&self, project_root: &Path) -> Result<()>;

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
