use std::path::Path;

use changeset_core::{Changeset, PackageInfo};
use indexmap::IndexMap;

use crate::Result;
use crate::traits::{ChangesetReader, ProjectProvider};

pub struct StatusOutput {
    pub changesets: Vec<Changeset>,
    pub projected_bumps: IndexMap<String, Vec<changeset_core::BumpType>>,
    pub unchanged_packages: Vec<PackageInfo>,
}

pub struct StatusOperation<P, R> {
    project_provider: P,
    changeset_reader: R,
}

impl<P, R> StatusOperation<P, R>
where
    P: ProjectProvider,
    R: ChangesetReader,
{
    pub fn new(project_provider: P, changeset_reader: R) -> Self {
        Self {
            project_provider,
            changeset_reader,
        }
    }

    /// # Errors
    ///
    /// Returns an error if the project cannot be discovered or if changeset files
    /// cannot be read.
    pub fn execute(&self, start_path: &Path) -> Result<StatusOutput> {
        let project = self.project_provider.discover_project(start_path)?;
        let (root_config, _) = self.project_provider.load_configs(&project)?;

        let changeset_dir = project.root.join(root_config.changeset_dir());
        let changeset_files = self.changeset_reader.list_changesets(&changeset_dir)?;

        let mut changesets = Vec::new();
        let mut projected_bumps: IndexMap<String, Vec<changeset_core::BumpType>> = IndexMap::new();

        for path in &changeset_files {
            let changeset = self.changeset_reader.read_changeset(path)?;
            for release in &changeset.releases {
                projected_bumps
                    .entry(release.name.clone())
                    .or_default()
                    .push(release.bump_type);
            }
            changesets.push(changeset);
        }

        let packages_with_changesets: std::collections::HashSet<_> =
            projected_bumps.keys().cloned().collect();

        let unchanged_packages: Vec<PackageInfo> = project
            .packages
            .into_iter()
            .filter(|p| !packages_with_changesets.contains(&p.name))
            .collect();

        Ok(StatusOutput {
            changesets,
            projected_bumps,
            unchanged_packages,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{MockChangesetReader, MockProjectProvider, make_changeset};
    use changeset_core::BumpType;
    use std::path::PathBuf;

    #[test]
    fn returns_empty_when_no_changesets() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset_reader = MockChangesetReader::new();

        let operation = StatusOperation::new(project_provider, changeset_reader);

        let result = operation
            .execute(Path::new("/any"))
            .expect("StatusOperation failed for project with no changesets");

        assert!(result.changesets.is_empty());
        assert!(result.projected_bumps.is_empty());
        assert_eq!(result.unchanged_packages.len(), 1);
        assert_eq!(result.unchanged_packages[0].name, "my-crate");
    }

    #[test]
    fn collects_changesets_and_projected_bumps() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");

        let changeset = make_changeset("my-crate", BumpType::Minor, "Add feature");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/test.md"), changeset);

        let operation = StatusOperation::new(project_provider, changeset_reader);

        let result = operation
            .execute(Path::new("/any"))
            .expect("StatusOperation failed to collect changesets");

        assert_eq!(result.changesets.len(), 1);
        assert!(result.projected_bumps.contains_key("my-crate"));
        assert_eq!(result.projected_bumps["my-crate"], vec![BumpType::Minor]);
        assert!(result.unchanged_packages.is_empty());
    }

    #[test]
    fn aggregates_multiple_changesets_for_same_package() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");

        let changeset1 = make_changeset("my-crate", BumpType::Patch, "Fix bug");
        let changeset2 = make_changeset("my-crate", BumpType::Minor, "Add feature");

        let changeset_reader = MockChangesetReader::new().with_changesets(vec![
            (PathBuf::from(".changeset/fix.md"), changeset1),
            (PathBuf::from(".changeset/feature.md"), changeset2),
        ]);

        let operation = StatusOperation::new(project_provider, changeset_reader);

        let result = operation
            .execute(Path::new("/any"))
            .expect("StatusOperation failed to aggregate multiple changesets");

        assert_eq!(result.changesets.len(), 2);
        assert_eq!(result.projected_bumps["my-crate"].len(), 2);
        assert!(result.projected_bumps["my-crate"].contains(&BumpType::Patch));
        assert!(result.projected_bumps["my-crate"].contains(&BumpType::Minor));
    }

    #[test]
    fn identifies_unchanged_packages_in_workspace() {
        let project_provider =
            MockProjectProvider::workspace(vec![("crate-a", "1.0.0"), ("crate-b", "2.0.0")]);

        let changeset = make_changeset("crate-a", BumpType::Patch, "Fix crate-a");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/test.md"), changeset);

        let operation = StatusOperation::new(project_provider, changeset_reader);

        let result = operation
            .execute(Path::new("/any"))
            .expect("StatusOperation failed to identify unchanged packages");

        assert_eq!(result.unchanged_packages.len(), 1);
        assert_eq!(result.unchanged_packages[0].name, "crate-b");
    }
}
