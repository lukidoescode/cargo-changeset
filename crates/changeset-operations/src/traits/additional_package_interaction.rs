use std::path::{Path, PathBuf};

use changeset_core::{AdditionalPackageDeclaration, ManifestFormat};

use crate::Result;
use crate::traits::MenuSelection;

#[derive(Debug, Clone)]
pub enum AdditionalPackageField {
    Path,
    Influence,
    ManifestFilePath,
    ManifestFormat,
    ManifestVersionFieldPath,
}

pub trait AdditionalPackageInteractionProvider: Send + Sync {
    /// # Errors
    /// Returns an error if the terminal interaction fails (e.g. I/O error or cancelled).
    fn prompt_package_name(&self) -> Result<String>;

    /// # Errors
    /// Returns an error if the terminal interaction fails.
    fn prompt_package_path(&self) -> Result<PathBuf>;

    /// # Errors
    /// Returns an error if the terminal interaction fails.
    fn prompt_influence_patterns(&self, package_path: &Path) -> Result<Vec<String>>;

    /// # Errors
    /// Returns an error if the terminal interaction fails.
    fn prompt_manifest_file_path(&self) -> Result<PathBuf>;

    /// # Errors
    /// Returns an error if the terminal interaction fails or the user cancels.
    fn prompt_manifest_format(&self) -> Result<ManifestFormat>;

    /// # Errors
    /// Returns an error if the terminal interaction fails.
    fn prompt_manifest_version_field_path(&self) -> Result<String>;

    /// # Errors
    /// Returns an error if the terminal interaction fails.
    fn select_package_to_remove(
        &self,
        packages: &[&AdditionalPackageDeclaration],
    ) -> Result<MenuSelection<usize>>;

    /// # Errors
    /// Returns an error if the terminal interaction fails.
    fn select_package_to_edit(
        &self,
        packages: &[&AdditionalPackageDeclaration],
    ) -> Result<MenuSelection<usize>>;

    /// # Errors
    /// Returns an error if the terminal interaction fails.
    fn select_field_to_edit(&self) -> Result<MenuSelection<AdditionalPackageField>>;

    /// # Errors
    /// Returns an error if the terminal interaction fails.
    fn confirm_removal(&self, name: &str) -> Result<bool>;
}
