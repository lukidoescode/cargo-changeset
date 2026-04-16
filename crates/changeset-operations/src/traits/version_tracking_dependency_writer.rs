use std::path::Path;

use changeset_core::VersionTrackingDependency;
use changeset_manifest::MetadataSection;

use crate::Result;

pub trait VersionTrackingDependencyWriter: Send + Sync {
    /// # Errors
    ///
    /// Returns an error if the manifest file cannot be read or written.
    fn add_dependency_to_additional_package(
        &self,
        manifest_path: &Path,
        section: MetadataSection,
        package_name: &str,
        dependency: &VersionTrackingDependency,
    ) -> Result<bool>;

    /// Returns `true` if the dependency was found and removed.
    ///
    /// # Errors
    ///
    /// Returns an error if the manifest file cannot be read or written.
    fn remove_dependency_from_additional_package(
        &self,
        manifest_path: &Path,
        section: MetadataSection,
        package_name: &str,
        dependency_name: &str,
    ) -> Result<bool>;

    /// # Errors
    ///
    /// Returns an error if the manifest file cannot be read or written.
    fn add_dependency_to_crate(
        &self,
        manifest_path: &Path,
        dependency: &VersionTrackingDependency,
    ) -> Result<()>;

    /// Returns `true` if the dependency was found and removed.
    ///
    /// # Errors
    ///
    /// Returns an error if the manifest file cannot be read or written.
    fn remove_dependency_from_crate(
        &self,
        manifest_path: &Path,
        dependency_name: &str,
    ) -> Result<bool>;
}
