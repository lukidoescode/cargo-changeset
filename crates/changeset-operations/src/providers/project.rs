use std::collections::HashMap;
use std::path::{Path, PathBuf};

use changeset_core::PackageInfo;
use changeset_project::{
    CargoProject, PackageChangesetConfig, RootChangesetConfig, WorkspaceDependencyGraph,
    discover_additional_packages, discover_project, ensure_changeset_dir, load_changeset_configs,
};

use crate::Result;
use crate::traits::{DependencyGraphProvider, ProjectProvider};

pub struct FileSystemProjectProvider;

impl FileSystemProjectProvider {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for FileSystemProjectProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectProvider for FileSystemProjectProvider {
    fn discover_project(&self, start_path: &Path) -> Result<CargoProject> {
        Ok(discover_project(start_path)?)
    }

    fn load_configs(
        &self,
        project: &CargoProject,
    ) -> Result<(RootChangesetConfig, HashMap<String, PackageChangesetConfig>)> {
        Ok(load_changeset_configs(project)?)
    }

    fn ensure_changeset_dir(
        &self,
        project: &CargoProject,
        config: &RootChangesetConfig,
    ) -> Result<PathBuf> {
        Ok(ensure_changeset_dir(project, config)?)
    }

    fn discover_additional_packages(
        &self,
        project_root: &Path,
        config: &RootChangesetConfig,
    ) -> Result<Vec<PackageInfo>> {
        Ok(discover_additional_packages(
            project_root,
            config.additional_packages(),
        )?)
    }
}

impl DependencyGraphProvider for FileSystemProjectProvider {
    fn build_dependency_graph(&self, project: &CargoProject) -> Result<WorkspaceDependencyGraph> {
        Ok(WorkspaceDependencyGraph::build(project)?)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use changeset_core::AdditionalPackageDeclaration;
    use changeset_project::RootChangesetConfig;
    use tempfile::TempDir;

    use super::FileSystemProjectProvider;
    use crate::traits::ProjectProvider;

    fn make_decl(json: &str) -> AdditionalPackageDeclaration {
        serde_json::from_str(json).expect("valid declaration JSON")
    }

    #[test]
    fn discover_additional_packages_returns_empty_for_no_declarations() -> Result<()> {
        let dir = TempDir::new()?;
        let config = RootChangesetConfig::default();
        let provider = FileSystemProjectProvider::new();

        let packages = provider.discover_additional_packages(dir.path(), &config)?;
        assert!(packages.is_empty());
        Ok(())
    }

    #[test]
    fn discover_additional_packages_discovers_yaml_package() -> Result<()> {
        let dir = TempDir::new()?;
        let root = dir.path();

        std::fs::create_dir_all(root.join("charts/my-chart"))?;
        std::fs::write(
            root.join("charts/my-chart/Chart.yaml"),
            "version: \"1.2.3\"\n",
        )?;

        let decl = make_decl(&format!(
            r#"{{
                "name": "my-helm-chart",
                "path": "charts/my-chart",
                "influence": ["charts/my-chart/**"],
                "manifest": {{
                    "file-path": "{}/charts/my-chart/Chart.yaml",
                    "format": "yaml",
                    "version-path": "version"
                }}
            }}"#,
            root.display()
        ));

        let config = RootChangesetConfig::default().with_additional_packages(vec![decl]);
        let provider = FileSystemProjectProvider::new();

        let packages = provider.discover_additional_packages(root, &config)?;
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name(), "my-helm-chart");
        assert_eq!(packages[0].version(), &semver::Version::new(1, 2, 3));
        Ok(())
    }

    #[test]
    fn discover_additional_packages_propagates_error_for_missing_manifest() -> Result<()> {
        let dir = TempDir::new()?;
        let root = dir.path();

        let decl = make_decl(
            r#"{
                "name": "missing-pkg",
                "path": "nonexistent",
                "influence": [],
                "manifest": {
                    "file-path": "/nonexistent/path/Chart.yaml",
                    "format": "yaml",
                    "version-path": "version"
                }
            }"#,
        );

        let config = RootChangesetConfig::default().with_additional_packages(vec![decl]);
        let provider = FileSystemProjectProvider::new();

        let result = provider.discover_additional_packages(root, &config);
        assert!(result.is_err());
        Ok(())
    }
}
