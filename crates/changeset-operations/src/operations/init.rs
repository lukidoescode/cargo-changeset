use std::path::{Path, PathBuf};

use crate::Result;
use crate::traits::ProjectProvider;

pub struct InitOutput {
    pub changeset_dir: PathBuf,
    pub created: bool,
}

pub struct InitOperation<P> {
    project_provider: P,
}

impl<P> InitOperation<P>
where
    P: ProjectProvider,
{
    pub fn new(project_provider: P) -> Self {
        Self { project_provider }
    }

    /// # Errors
    ///
    /// Returns an error if the project cannot be discovered or the changeset
    /// directory cannot be created.
    pub fn execute(&self, start_path: &Path) -> Result<InitOutput> {
        let project = self.project_provider.discover_project(start_path)?;
        let (root_config, _) = self.project_provider.load_configs(&project)?;

        let changeset_dir_path = root_config.changeset_dir();
        let existed = project.root.join(changeset_dir_path).exists();

        let changeset_dir = self
            .project_provider
            .ensure_changeset_dir(&project, &root_config)?;

        Ok(InitOutput {
            changeset_dir,
            created: !existed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::MockProjectProvider;

    #[test]
    fn returns_changeset_dir_path() {
        let changeset_dir = PathBuf::from("/mock/project/.changeset");
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0")
            .with_changeset_dir(changeset_dir.clone());

        let operation = InitOperation::new(project_provider);

        let result = operation
            .execute(Path::new("/any"))
            .expect("InitOperation failed for single-package project");

        assert_eq!(result.changeset_dir, changeset_dir);
    }

    #[test]
    fn works_with_workspace_projects() {
        let project_provider =
            MockProjectProvider::workspace(vec![("crate-a", "1.0.0"), ("crate-b", "2.0.0")]);

        let operation = InitOperation::new(project_provider);

        let result = operation
            .execute(Path::new("/any"))
            .expect("InitOperation failed for workspace project");

        assert!(
            result
                .changeset_dir
                .to_string_lossy()
                .contains(".changeset")
        );
    }
}
