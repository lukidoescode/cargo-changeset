use std::path::Path;

use changeset_core::AdditionalPackageDeclaration;
use changeset_manifest::{AdditionalPackageUpdate, MetadataSection};

use crate::Result;

pub trait AdditionalPackageConfigWriter: Send + Sync {
    /// # Errors
    ///
    /// Returns an error if the manifest file cannot be read or written.
    fn add_additional_package(
        &self,
        manifest_path: &Path,
        section: MetadataSection,
        declaration: &AdditionalPackageDeclaration,
    ) -> Result<()>;

    /// Returns `true` if the entry was found and removed, `false` if no entry matched `name`.
    ///
    /// # Errors
    ///
    /// Returns an error if the manifest file cannot be read or written.
    fn remove_additional_package(
        &self,
        manifest_path: &Path,
        section: MetadataSection,
        name: &str,
    ) -> Result<bool>;

    /// Returns `true` if the entry was found and updated, `false` if no entry matched `name`.
    ///
    /// # Errors
    ///
    /// Returns an error if the manifest file cannot be read or written.
    fn update_additional_package(
        &self,
        manifest_path: &Path,
        section: MetadataSection,
        name: &str,
        updates: &AdditionalPackageUpdate,
    ) -> Result<bool>;
}
