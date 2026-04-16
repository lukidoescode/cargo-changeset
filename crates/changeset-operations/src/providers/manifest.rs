use std::path::Path;
use std::process::Command;

use changeset_core::{AdditionalPackageDeclaration, ManifestFormat};
use changeset_manifest::{AdditionalPackageUpdate, InitConfig, MetadataSection};
use semver::Version;

use crate::Result;
use crate::error::OperationError;
use changeset_core::VersionTrackingDependency;

use crate::traits::{
    AdditionalPackageConfigWriter, ExternalManifestVersionReader, ExternalManifestVersionWriter,
    InheritedVersionChecker, LockfileUpdater, ManifestDependencyWriter, ManifestMetadataWriter,
    ManifestVersionWriter, VersionTrackingDependencyWriter, WorkspaceVersionManager,
};

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

impl ManifestVersionWriter for FileSystemManifestWriter {
    fn write_version(&self, manifest_path: &Path, new_version: &Version) -> Result<()> {
        Ok(changeset_manifest::write_version(
            manifest_path,
            new_version,
        )?)
    }

    fn verify_version(&self, manifest_path: &Path, expected: &Version) -> Result<()> {
        Ok(changeset_manifest::verify_version(manifest_path, expected)?)
    }
}

impl ManifestDependencyWriter for FileSystemManifestWriter {
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

impl WorkspaceVersionManager for FileSystemManifestWriter {
    fn read_workspace_version(&self, manifest_path: &Path) -> Result<Option<Version>> {
        match changeset_manifest::read_workspace_version(manifest_path) {
            Ok(version) => Ok(Some(version)),
            Err(changeset_manifest::ManifestError::MissingField { .. }) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn remove_workspace_version(&self, manifest_path: &Path) -> Result<()> {
        Ok(changeset_manifest::remove_workspace_version(manifest_path)?)
    }

    fn write_workspace_version(&self, manifest_path: &Path, version: &Version) -> Result<()> {
        Ok(changeset_manifest::write_workspace_version(
            manifest_path,
            version,
        )?)
    }
}

impl LockfileUpdater for FileSystemManifestWriter {
    fn update_lockfile(&self, project_root: &Path) -> Result<()> {
        let output = Command::new("cargo")
            .args(["update", "--workspace"])
            .current_dir(project_root)
            .output()
            .map_err(|source| OperationError::LockfileGeneration {
                path: project_root.to_path_buf(),
                source,
            })?;

        if !output.status.success() {
            return Err(OperationError::LockfileCommandFailed {
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }

        Ok(())
    }

    fn read_lockfile(&self, project_root: &Path) -> Result<Option<Vec<u8>>> {
        let lockfile_path = project_root.join("Cargo.lock");
        if lockfile_path.exists() {
            let content =
                std::fs::read(&lockfile_path).map_err(|source| OperationError::LockfileRead {
                    path: lockfile_path,
                    source,
                })?;
            Ok(Some(content))
        } else {
            Ok(None)
        }
    }

    fn restore_lockfile(&self, project_root: &Path, content: &[u8]) -> Result<()> {
        let lockfile_path = project_root.join("Cargo.lock");
        std::fs::write(&lockfile_path, content).map_err(|source| {
            OperationError::LockfileWrite {
                path: lockfile_path,
                source,
            }
        })?;
        Ok(())
    }

    fn remove_lockfile(&self, project_root: &Path) -> Result<()> {
        let lockfile_path = project_root.join("Cargo.lock");
        if lockfile_path.exists() {
            std::fs::remove_file(&lockfile_path).map_err(|source| {
                OperationError::LockfileWrite {
                    path: lockfile_path,
                    source,
                }
            })?;
        }
        Ok(())
    }
}

impl ExternalManifestVersionReader for FileSystemManifestWriter {
    fn read_external_version(
        &self,
        manifest_path: &Path,
        format: ManifestFormat,
        version_field_path: &str,
    ) -> Result<String> {
        Ok(changeset_manifest::read_external_version_string(
            manifest_path,
            format,
            version_field_path,
        )?)
    }
}

impl ExternalManifestVersionWriter for FileSystemManifestWriter {
    fn write_external_version(
        &self,
        manifest_path: &Path,
        format: ManifestFormat,
        version_field_path: &str,
        new_version: &Version,
    ) -> Result<()> {
        Ok(changeset_manifest::write_external_version(
            manifest_path,
            format,
            version_field_path,
            new_version,
        )?)
    }

    fn verify_external_version(
        &self,
        manifest_path: &Path,
        format: ManifestFormat,
        version_field_path: &str,
        expected: &Version,
    ) -> Result<()> {
        Ok(changeset_manifest::verify_external_version(
            manifest_path,
            format,
            version_field_path,
            expected,
        )?)
    }

    fn restore_external_version(
        &self,
        manifest_path: &Path,
        format: ManifestFormat,
        version_field_path: &str,
        version_str: &str,
    ) -> Result<()> {
        Ok(changeset_manifest::restore_external_version(
            manifest_path,
            format,
            version_field_path,
            version_str,
        )?)
    }
}

impl ManifestMetadataWriter for FileSystemManifestWriter {
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
}

impl VersionTrackingDependencyWriter for FileSystemManifestWriter {
    fn add_dependency_to_additional_package(
        &self,
        manifest_path: &Path,
        section: MetadataSection,
        package_name: &str,
        dependency: &VersionTrackingDependency,
    ) -> Result<bool> {
        Ok(
            changeset_manifest::add_version_tracking_dependency_to_additional_package(
                manifest_path,
                section,
                package_name,
                dependency,
            )?,
        )
    }

    fn remove_dependency_from_additional_package(
        &self,
        manifest_path: &Path,
        section: MetadataSection,
        package_name: &str,
        dependency_name: &str,
    ) -> Result<bool> {
        Ok(
            changeset_manifest::remove_version_tracking_dependency_from_additional_package(
                manifest_path,
                section,
                package_name,
                dependency_name,
            )?,
        )
    }

    fn add_dependency_to_crate(
        &self,
        manifest_path: &Path,
        dependency: &VersionTrackingDependency,
    ) -> Result<()> {
        Ok(
            changeset_manifest::add_version_tracking_dependency_to_crate(
                manifest_path,
                dependency,
            )?,
        )
    }

    fn remove_dependency_from_crate(
        &self,
        manifest_path: &Path,
        dependency_name: &str,
    ) -> Result<bool> {
        Ok(
            changeset_manifest::remove_version_tracking_dependency_from_crate(
                manifest_path,
                dependency_name,
            )?,
        )
    }
}

impl AdditionalPackageConfigWriter for FileSystemManifestWriter {
    fn add_additional_package(
        &self,
        manifest_path: &Path,
        section: MetadataSection,
        declaration: &AdditionalPackageDeclaration,
    ) -> Result<()> {
        Ok(changeset_manifest::add_additional_package(
            manifest_path,
            section,
            declaration,
        )?)
    }

    fn remove_additional_package(
        &self,
        manifest_path: &Path,
        section: MetadataSection,
        name: &str,
    ) -> Result<bool> {
        Ok(changeset_manifest::remove_additional_package(
            manifest_path,
            section,
            name,
        )?)
    }

    fn update_additional_package(
        &self,
        manifest_path: &Path,
        section: MetadataSection,
        name: &str,
        updates: &AdditionalPackageUpdate,
    ) -> Result<bool> {
        Ok(changeset_manifest::update_additional_package(
            manifest_path,
            section,
            name,
            updates,
        )?)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use changeset_core::ManifestFormat;
    use semver::Version;
    use tempfile::NamedTempFile;

    use super::FileSystemManifestWriter;
    use crate::traits::ExternalManifestVersionWriter;

    fn toml_manifest(version: &str) -> String {
        format!("[package]\nversion = \"{version}\"\n")
    }

    #[test]
    fn write_external_version_writes_toml_version() -> Result<()> {
        let file = NamedTempFile::new()?;
        std::fs::write(file.path(), toml_manifest("1.0.0"))?;

        let writer = FileSystemManifestWriter::new();
        writer.write_external_version(
            file.path(),
            ManifestFormat::Toml,
            "package.version",
            &Version::new(2, 0, 0),
        )?;

        let content = std::fs::read_to_string(file.path())?;
        assert!(content.contains("\"2.0.0\""));
        Ok(())
    }

    #[test]
    fn verify_external_version_succeeds_when_matching() -> Result<()> {
        let file = NamedTempFile::new()?;
        std::fs::write(file.path(), toml_manifest("1.0.0"))?;

        let writer = FileSystemManifestWriter::new();
        writer.verify_external_version(
            file.path(),
            ManifestFormat::Toml,
            "package.version",
            &Version::new(1, 0, 0),
        )?;
        Ok(())
    }

    #[test]
    fn verify_external_version_fails_when_mismatched() -> Result<()> {
        let file = NamedTempFile::new()?;
        std::fs::write(file.path(), toml_manifest("1.0.0"))?;

        let writer = FileSystemManifestWriter::new();
        let result = writer.verify_external_version(
            file.path(),
            ManifestFormat::Toml,
            "package.version",
            &Version::new(2, 0, 0),
        );
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn write_external_version_propagates_error_for_missing_file() {
        let writer = FileSystemManifestWriter::new();
        let result = writer.write_external_version(
            std::path::Path::new("/nonexistent/path/manifest.toml"),
            ManifestFormat::Toml,
            "package.version",
            &Version::new(1, 0, 0),
        );
        assert!(result.is_err());
    }
}
