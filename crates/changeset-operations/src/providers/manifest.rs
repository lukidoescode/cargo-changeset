use std::path::Path;
use std::process::Command;

use changeset_manifest::{InitConfig, MetadataSection};
use semver::Version;

use crate::Result;
use crate::error::OperationError;
use crate::traits::{
    InheritedVersionChecker, LockfileUpdater, ManifestDependencyWriter, ManifestMetadataWriter,
    ManifestVersionWriter, WorkspaceVersionManager,
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
