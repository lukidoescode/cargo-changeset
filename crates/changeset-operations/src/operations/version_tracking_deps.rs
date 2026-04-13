use std::path::{Path, PathBuf};

use changeset_core::{
    CARGO_MANIFEST_FILENAME, ManifestFormat, VersionTrackingDependency, VersionTrackingManifest,
};
use changeset_manifest::MetadataSection;
use changeset_project::ProjectKind;
use gset::Getset;

use crate::Result;
use crate::error::OperationError;
use crate::traits::{ProjectProvider, VersionTrackingDependencyWriter};

pub struct VersionTrackingDependencyAddInput {
    package_name: String,
    dependency_name: String,
    manifest_file_path: PathBuf,
    manifest_format: ManifestFormat,
    version_field_path: String,
}

impl VersionTrackingDependencyAddInput {
    #[must_use]
    pub fn new(
        package_name: String,
        dependency_name: String,
        manifest_file_path: PathBuf,
        manifest_format: ManifestFormat,
        version_field_path: String,
    ) -> Self {
        Self {
            package_name,
            dependency_name,
            manifest_file_path,
            manifest_format,
            version_field_path,
        }
    }
}

pub struct VersionTrackingDependencyRemoveInput {
    package_name: String,
    dependency_name: String,
}

impl VersionTrackingDependencyRemoveInput {
    #[must_use]
    pub fn new(package_name: String, dependency_name: String) -> Self {
        Self {
            package_name,
            dependency_name,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionTrackingDependencyEvent {
    Added {
        package_name: String,
        dependency_name: String,
    },
    Removed {
        package_name: String,
        dependency_name: String,
    },
    Listed(Vec<VersionTrackingDependencySummary>),
    NoDependencies {
        package_name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Getset)]
pub struct VersionTrackingDependencySummary {
    #[getset(get, vis = "pub")]
    dependency_name: String,
    #[getset(get, vis = "pub")]
    manifest_file_path: PathBuf,
    #[getset(get_copy, vis = "pub")]
    manifest_format: ManifestFormat,
    #[getset(get, vis = "pub")]
    version_field_path: String,
}

pub struct VersionTrackingDependencyAddOperation<P, W> {
    project_provider: P,
    writer: W,
}

impl<P, W> VersionTrackingDependencyAddOperation<P, W>
where
    P: ProjectProvider,
    W: VersionTrackingDependencyWriter,
{
    #[must_use]
    pub fn new(project_provider: P, writer: W) -> Self {
        Self {
            project_provider,
            writer,
        }
    }

    /// # Errors
    ///
    /// Returns an error if the project cannot be discovered, configs cannot be loaded,
    /// or the manifest cannot be written.
    pub fn execute(
        &self,
        start_path: &Path,
        input: VersionTrackingDependencyAddInput,
    ) -> Result<Vec<VersionTrackingDependencyEvent>> {
        let project = self.project_provider.discover_project(start_path)?;
        let (root_config, _) = self.project_provider.load_configs(&project)?;

        let dependency = VersionTrackingDependency::new(
            input.dependency_name.clone(),
            VersionTrackingManifest::new(
                input.manifest_file_path,
                input.manifest_format,
                input.version_field_path,
            ),
        );

        let is_additional = root_config
            .additional_packages()
            .iter()
            .any(|p| p.name() == &input.package_name);

        let is_crate = project
            .packages()
            .iter()
            .any(|p| p.name() == &input.package_name);

        if is_additional {
            let (manifest_path, section) = resolve_workspace_manifest(&project);
            self.writer.add_dependency_to_additional_package(
                &manifest_path,
                section,
                &input.package_name,
                &dependency,
            )?;
        } else if is_crate {
            let crate_manifest = find_crate_manifest(&project, &input.package_name)?;
            self.writer
                .add_dependency_to_crate(&crate_manifest, &dependency)?;
        } else {
            return Err(OperationError::PackageNotFound {
                name: input.package_name,
            });
        }

        Ok(vec![VersionTrackingDependencyEvent::Added {
            package_name: input.package_name,
            dependency_name: input.dependency_name,
        }])
    }
}

pub struct VersionTrackingDependencyRemoveOperation<P, W> {
    project_provider: P,
    writer: W,
}

impl<P, W> VersionTrackingDependencyRemoveOperation<P, W>
where
    P: ProjectProvider,
    W: VersionTrackingDependencyWriter,
{
    #[must_use]
    pub fn new(project_provider: P, writer: W) -> Self {
        Self {
            project_provider,
            writer,
        }
    }

    /// # Errors
    ///
    /// Returns an error if the project cannot be discovered, configs cannot be loaded,
    /// or the manifest cannot be written.
    pub fn execute(
        &self,
        start_path: &Path,
        input: VersionTrackingDependencyRemoveInput,
    ) -> Result<Vec<VersionTrackingDependencyEvent>> {
        let project = self.project_provider.discover_project(start_path)?;
        let (root_config, _) = self.project_provider.load_configs(&project)?;

        let is_additional = root_config
            .additional_packages()
            .iter()
            .any(|p| p.name() == &input.package_name);

        let is_crate = project
            .packages()
            .iter()
            .any(|p| p.name() == &input.package_name);

        if is_additional {
            let (manifest_path, section) = resolve_workspace_manifest(&project);
            self.writer.remove_dependency_from_additional_package(
                &manifest_path,
                section,
                &input.package_name,
                &input.dependency_name,
            )?;
        } else if is_crate {
            let crate_manifest = find_crate_manifest(&project, &input.package_name)?;
            self.writer
                .remove_dependency_from_crate(&crate_manifest, &input.dependency_name)?;
        } else {
            return Err(OperationError::PackageNotFound {
                name: input.package_name,
            });
        }

        Ok(vec![VersionTrackingDependencyEvent::Removed {
            package_name: input.package_name,
            dependency_name: input.dependency_name,
        }])
    }
}

pub struct VersionTrackingDependencyListOperation<P> {
    project_provider: P,
}

impl<P> VersionTrackingDependencyListOperation<P>
where
    P: ProjectProvider,
{
    #[must_use]
    pub fn new(project_provider: P) -> Self {
        Self { project_provider }
    }

    /// # Errors
    ///
    /// Returns an error if the project cannot be discovered or configs cannot be loaded.
    pub fn execute(
        &self,
        start_path: &Path,
        package_name: &str,
    ) -> Result<Vec<VersionTrackingDependencyEvent>> {
        let project = self.project_provider.discover_project(start_path)?;
        let (root_config, package_configs) = self.project_provider.load_configs(&project)?;

        let additional_pkg = root_config
            .additional_packages()
            .iter()
            .find(|p| p.name() == package_name);

        if let Some(pkg) = additional_pkg {
            let deps = pkg.dependencies();
            if deps.is_empty() {
                return Ok(vec![VersionTrackingDependencyEvent::NoDependencies {
                    package_name: package_name.to_string(),
                }]);
            }
            let summaries = deps
                .iter()
                .map(|d| VersionTrackingDependencySummary {
                    dependency_name: d.dependency_name().clone(),
                    manifest_file_path: d.version_tracking_manifest().file_path().clone(),
                    manifest_format: d.version_tracking_manifest().format(),
                    version_field_path: d.version_tracking_manifest().version_field_path().clone(),
                })
                .collect();
            return Ok(vec![VersionTrackingDependencyEvent::Listed(summaries)]);
        }

        let is_crate = project.packages().iter().any(|p| p.name() == package_name);

        if is_crate {
            let deps = package_configs
                .get(package_name)
                .map(changeset_project::PackageChangesetConfig::additional_package_dependencies)
                .unwrap_or_default();

            if deps.is_empty() {
                return Ok(vec![VersionTrackingDependencyEvent::NoDependencies {
                    package_name: package_name.to_string(),
                }]);
            }

            let summaries = deps
                .iter()
                .map(|d| VersionTrackingDependencySummary {
                    dependency_name: d.dependency_name().clone(),
                    manifest_file_path: d.version_tracking_manifest().file_path().clone(),
                    manifest_format: d.version_tracking_manifest().format(),
                    version_field_path: d.version_tracking_manifest().version_field_path().clone(),
                })
                .collect();
            return Ok(vec![VersionTrackingDependencyEvent::Listed(summaries)]);
        }

        Err(OperationError::PackageNotFound {
            name: package_name.to_string(),
        })
    }
}

fn resolve_workspace_manifest(
    project: &changeset_project::CargoProject,
) -> (PathBuf, MetadataSection) {
    let manifest_path = project.root().join(CARGO_MANIFEST_FILENAME);
    let section = match project.kind() {
        ProjectKind::VirtualWorkspace | ProjectKind::WorkspaceWithRoot => {
            MetadataSection::Workspace
        }
        ProjectKind::SinglePackage => MetadataSection::Package,
    };
    (manifest_path, section)
}

fn find_crate_manifest(
    project: &changeset_project::CargoProject,
    package_name: &str,
) -> Result<PathBuf> {
    project
        .packages()
        .iter()
        .find(|p| p.name() == package_name)
        .map(|p| p.path().join(CARGO_MANIFEST_FILENAME))
        .ok_or_else(|| OperationError::PackageNotFound {
            name: package_name.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use changeset_core::{
        AdditionalPackageDeclaration, AdditionalPackageManifest, ManifestFormat,
        VersionTrackingDependency, VersionTrackingManifest,
    };
    use changeset_project::RootChangesetConfig;

    use super::*;
    use crate::mocks::{MockManifestWriter, MockProjectProvider};

    fn make_decl_with_deps(
        name: &str,
        deps: Vec<VersionTrackingDependency>,
    ) -> AdditionalPackageDeclaration {
        AdditionalPackageDeclaration::new(
            name.to_string(),
            PathBuf::from(format!("charts/{name}")),
            vec![format!("charts/{name}/**")],
            AdditionalPackageManifest::new(
                PathBuf::from(format!("charts/{name}/Chart.yaml")),
                ManifestFormat::Yaml,
                "version".to_string(),
            ),
            deps,
        )
    }

    fn make_tracking_dep(dep_name: &str) -> VersionTrackingDependency {
        VersionTrackingDependency::new(
            dep_name.to_string(),
            VersionTrackingManifest::new(
                PathBuf::from(format!("charts/dep/{dep_name}.yaml")),
                ManifestFormat::Yaml,
                "appVersion".to_string(),
            ),
        )
    }

    #[test]
    fn add_dependency_to_additional_package_succeeds() {
        let config = RootChangesetConfig::default()
            .with_additional_packages(vec![make_decl_with_deps("my-chart", vec![])]);
        let provider =
            MockProjectProvider::workspace(vec![("crate-a", "1.0.0")]).with_root_config(config);
        let writer = MockManifestWriter::new();

        let op = VersionTrackingDependencyAddOperation::new(provider, writer);
        let input = VersionTrackingDependencyAddInput::new(
            "my-chart".to_string(),
            "crate-a".to_string(),
            PathBuf::from("charts/dep/crate-a.yaml"),
            ManifestFormat::Yaml,
            "appVersion".to_string(),
        );

        let events = op
            .execute(Path::new("/any"), input)
            .expect("should succeed");
        assert!(events.iter().any(|e| matches!(
            e,
            VersionTrackingDependencyEvent::Added {
                package_name,
                dependency_name
            } if package_name == "my-chart" && dependency_name == "crate-a"
        )));
    }

    #[test]
    fn add_dependency_to_crate_succeeds() {
        let provider = MockProjectProvider::workspace(vec![("crate-a", "1.0.0")]);
        let writer = MockManifestWriter::new();

        let op = VersionTrackingDependencyAddOperation::new(provider, writer);
        let input = VersionTrackingDependencyAddInput::new(
            "crate-a".to_string(),
            "my-chart".to_string(),
            PathBuf::from("charts/dep/my-chart.yaml"),
            ManifestFormat::Yaml,
            "appVersion".to_string(),
        );

        let events = op
            .execute(Path::new("/any"), input)
            .expect("should succeed");
        assert!(events.iter().any(|e| matches!(
            e,
            VersionTrackingDependencyEvent::Added {
                package_name,
                dependency_name
            } if package_name == "crate-a" && dependency_name == "my-chart"
        )));
    }

    #[test]
    fn add_dependency_to_unknown_package_fails() {
        let provider = MockProjectProvider::workspace(vec![("crate-a", "1.0.0")]);
        let writer = MockManifestWriter::new();

        let op = VersionTrackingDependencyAddOperation::new(provider, writer);
        let input = VersionTrackingDependencyAddInput::new(
            "nonexistent".to_string(),
            "crate-a".to_string(),
            PathBuf::from("charts/dep/crate-a.yaml"),
            ManifestFormat::Yaml,
            "appVersion".to_string(),
        );

        let result = op.execute(Path::new("/any"), input);
        assert!(matches!(
            result,
            Err(OperationError::PackageNotFound { .. })
        ));
    }

    #[test]
    fn remove_dependency_from_additional_package_succeeds() {
        let config =
            RootChangesetConfig::default().with_additional_packages(vec![make_decl_with_deps(
                "my-chart",
                vec![make_tracking_dep("crate-a")],
            )]);
        let provider =
            MockProjectProvider::workspace(vec![("crate-a", "1.0.0")]).with_root_config(config);
        let writer = MockManifestWriter::new();

        let op = VersionTrackingDependencyRemoveOperation::new(provider, writer);
        let input = VersionTrackingDependencyRemoveInput::new(
            "my-chart".to_string(),
            "crate-a".to_string(),
        );

        let events = op
            .execute(Path::new("/any"), input)
            .expect("should succeed");
        assert!(events.iter().any(|e| matches!(
            e,
            VersionTrackingDependencyEvent::Removed {
                package_name,
                dependency_name
            } if package_name == "my-chart" && dependency_name == "crate-a"
        )));
    }

    #[test]
    fn remove_dependency_from_unknown_package_fails() {
        let provider = MockProjectProvider::workspace(vec![("crate-a", "1.0.0")]);
        let writer = MockManifestWriter::new();

        let op = VersionTrackingDependencyRemoveOperation::new(provider, writer);
        let input = VersionTrackingDependencyRemoveInput::new(
            "nonexistent".to_string(),
            "crate-a".to_string(),
        );

        let result = op.execute(Path::new("/any"), input);
        assert!(matches!(
            result,
            Err(OperationError::PackageNotFound { .. })
        ));
    }

    #[test]
    fn list_dependencies_for_additional_package() {
        let dep = make_tracking_dep("crate-a");
        let config = RootChangesetConfig::default()
            .with_additional_packages(vec![make_decl_with_deps("my-chart", vec![dep])]);
        let provider =
            MockProjectProvider::workspace(vec![("crate-a", "1.0.0")]).with_root_config(config);

        let op = VersionTrackingDependencyListOperation::new(provider);
        let events = op
            .execute(Path::new("/any"), "my-chart")
            .expect("should succeed");

        let listed = events.iter().find_map(|e| {
            if let VersionTrackingDependencyEvent::Listed(s) = e {
                Some(s)
            } else {
                None
            }
        });
        let summaries = listed.expect("should have Listed event");
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].dependency_name(), "crate-a");
    }

    #[test]
    fn list_dependencies_for_package_with_none_returns_no_dependencies() {
        let config = RootChangesetConfig::default()
            .with_additional_packages(vec![make_decl_with_deps("my-chart", vec![])]);
        let provider =
            MockProjectProvider::workspace(vec![("crate-a", "1.0.0")]).with_root_config(config);

        let op = VersionTrackingDependencyListOperation::new(provider);
        let events = op
            .execute(Path::new("/any"), "my-chart")
            .expect("should succeed");
        assert!(events.iter().any(|e| matches!(
            e,
            VersionTrackingDependencyEvent::NoDependencies { package_name } if package_name == "my-chart"
        )));
    }

    #[test]
    fn list_dependencies_for_unknown_package_fails() {
        let provider = MockProjectProvider::workspace(vec![("crate-a", "1.0.0")]);
        let op = VersionTrackingDependencyListOperation::new(provider);

        let result = op.execute(Path::new("/any"), "nonexistent");
        assert!(matches!(
            result,
            Err(OperationError::PackageNotFound { .. })
        ));
    }
}
