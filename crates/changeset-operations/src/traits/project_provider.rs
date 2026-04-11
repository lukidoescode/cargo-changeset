use std::collections::HashMap;
use std::path::{Path, PathBuf};

use changeset_core::PackageInfo;
use changeset_project::{
    CargoProject, PackageChangesetConfig, RootChangesetConfig, WorkspaceDependencyGraph,
};

use crate::Result;

pub trait ProjectProvider: Send + Sync {
    /// # Errors
    ///
    /// Returns an error if no project can be found from the given path.
    fn discover_project(&self, start_path: &Path) -> Result<CargoProject>;

    /// # Errors
    ///
    /// Returns an error if the configuration files cannot be loaded.
    fn load_configs(
        &self,
        project: &CargoProject,
    ) -> Result<(RootChangesetConfig, HashMap<String, PackageChangesetConfig>)>;

    /// # Errors
    ///
    /// Returns an error if the changeset directory cannot be created.
    fn ensure_changeset_dir(
        &self,
        project: &CargoProject,
        config: &RootChangesetConfig,
    ) -> Result<PathBuf>;

    /// # Errors
    ///
    /// Returns an error if additional package manifests cannot be read.
    fn discover_additional_packages(
        &self,
        project_root: &Path,
        config: &RootChangesetConfig,
    ) -> Result<Vec<PackageInfo>>;
}

pub trait DependencyGraphProvider: Send + Sync {
    /// # Errors
    ///
    /// Returns an error if the dependency graph cannot be built from the project manifests.
    fn build_dependency_graph(&self, project: &CargoProject) -> Result<WorkspaceDependencyGraph>;
}
