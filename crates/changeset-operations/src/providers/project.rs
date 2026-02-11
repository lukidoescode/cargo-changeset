use std::collections::HashMap;
use std::path::{Path, PathBuf};

use changeset_project::{
    CargoProject, PackageChangesetConfig, RootChangesetConfig, discover_project,
    ensure_changeset_dir, load_changeset_configs,
};

use crate::Result;
use crate::traits::ProjectProvider;

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
}
