use std::path::Path;

use semver::Version;

use crate::Result;
use crate::traits::{InheritedVersionChecker, ManifestWriter};

pub struct FileSystemManifestWriter;

impl FileSystemManifestWriter {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for FileSystemManifestWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl InheritedVersionChecker for FileSystemManifestWriter {
    fn has_inherited_version(&self, manifest_path: &Path) -> Result<bool> {
        Ok(changeset_manifest::has_inherited_version(manifest_path)?)
    }
}

impl ManifestWriter for FileSystemManifestWriter {
    fn write_version(&self, manifest_path: &Path, new_version: &Version) -> Result<()> {
        Ok(changeset_manifest::write_version(
            manifest_path,
            new_version,
        )?)
    }

    fn remove_workspace_version(&self, manifest_path: &Path) -> Result<()> {
        Ok(changeset_manifest::remove_workspace_version(manifest_path)?)
    }

    fn verify_version(&self, manifest_path: &Path, expected: &Version) -> Result<()> {
        Ok(changeset_manifest::verify_version(manifest_path, expected)?)
    }
}
