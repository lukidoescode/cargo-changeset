use std::path::Path;

use changeset_manifest::{InitConfig, MetadataSection};
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

    fn read_workspace_version(&self, manifest_path: &Path) -> Result<Option<Version>> {
        match changeset_manifest::read_workspace_version(manifest_path) {
            Ok(version) => Ok(Some(version)),
            Err(changeset_manifest::ManifestError::MissingField { .. }) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn write_workspace_version(&self, manifest_path: &Path, version: &Version) -> Result<()> {
        Ok(changeset_manifest::write_workspace_version(
            manifest_path,
            version,
        )?)
    }

    fn verify_version(&self, manifest_path: &Path, expected: &Version) -> Result<()> {
        Ok(changeset_manifest::verify_version(manifest_path, expected)?)
    }

    fn write_metadata(
        &self,
        manifest_path: &Path,
        section: MetadataSection,
        config: &InitConfig,
    ) -> Result<()> {
        Ok(changeset_manifest::write_metadata_section(
            manifest_path,
            section,
            config,
        )?)
    }

    fn update_dependency_version(
        &self,
        manifest_path: &Path,
        dependency_name: &str,
        new_version: &Version,
    ) -> Result<bool> {
        Ok(changeset_manifest::update_dependency_version(
            manifest_path,
            dependency_name,
            new_version,
        )?)
    }
}
