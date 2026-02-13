use std::path::{Path, PathBuf};

use changeset_core::{BumpType, Changeset, PackageInfo};
use indexmap::IndexMap;

use super::release::PackageVersion;
use super::version_planner::VersionPlanner;
use crate::Result;
use crate::traits::{ChangesetReader, InheritedVersionChecker, ProjectProvider};

pub struct StatusOutput {
    /// All parsed changesets.
    pub changesets: Vec<Changeset>,
    /// Paths to changeset files.
    pub changeset_files: Vec<PathBuf>,
    /// Calculated releases (same type as `ReleaseOperation` uses).
    pub projected_releases: Vec<PackageVersion>,
    /// Raw bump types per package (for verbose display).
    pub bumps_by_package: IndexMap<String, Vec<BumpType>>,
    /// Packages with no pending changesets.
    pub unchanged_packages: Vec<PackageInfo>,
    /// Packages using inherited versions (informational warning).
    pub packages_with_inherited_versions: Vec<String>,
    /// Packages referenced in changesets but not in workspace.
    pub unknown_packages: Vec<String>,
    /// Changesets consumed for pre-release versions (path, version consumed for).
    pub consumed_prerelease_changesets: Vec<(PathBuf, String)>,
}

pub struct StatusOperation<P, R, I> {
    project_provider: P,
    changeset_reader: R,
    inherited_checker: I,
}

impl<P, R, I> StatusOperation<P, R, I>
where
    P: ProjectProvider,
    R: ChangesetReader,
    I: InheritedVersionChecker,
{
    pub fn new(project_provider: P, changeset_reader: R, inherited_checker: I) -> Self {
        Self {
            project_provider,
            changeset_reader,
            inherited_checker,
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
        for path in &changeset_files {
            let changeset = self.changeset_reader.read_changeset(path)?;
            changesets.push(changeset);
        }

        let consumed_changeset_paths = self
            .changeset_reader
            .list_consumed_changesets(&changeset_dir)?;
        let consumed_prerelease_changesets =
            Self::collect_consumed_changesets(&self.changeset_reader, &consumed_changeset_paths)?;

        let bumps_by_package = VersionPlanner::aggregate_bumps(&changesets);

        let plan = VersionPlanner::plan_releases(&changesets, &project.packages)?;

        let (_, unchanged_packages) =
            VersionPlanner::partition_packages(&changesets, &project.packages);

        let packages_with_inherited_versions = self
            .inherited_checker
            .find_packages_with_inherited_versions(&project.packages)?;

        Ok(StatusOutput {
            changesets,
            changeset_files,
            projected_releases: plan.releases,
            bumps_by_package,
            unchanged_packages,
            packages_with_inherited_versions,
            unknown_packages: plan.unknown_packages,
            consumed_prerelease_changesets,
        })
    }

    fn collect_consumed_changesets(
        reader: &R,
        paths: &[PathBuf],
    ) -> Result<Vec<(PathBuf, String)>> {
        let mut consumed = Vec::new();
        for path in paths {
            let changeset = reader.read_changeset(path)?;
            if let Some(version) = changeset.consumed_for_prerelease {
                consumed.push((path.clone(), version));
            }
        }
        Ok(consumed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mocks::{
        FailingInheritedVersionChecker, MockChangesetReader, MockInheritedVersionChecker,
        MockProjectProvider, make_changeset,
    };
    use changeset_core::BumpType;
    use semver::Version;
    use std::path::PathBuf;

    fn make_operation<P, R>(
        project_provider: P,
        changeset_reader: R,
    ) -> StatusOperation<P, R, MockInheritedVersionChecker>
    where
        P: ProjectProvider,
        R: ChangesetReader,
    {
        StatusOperation::new(
            project_provider,
            changeset_reader,
            MockInheritedVersionChecker::new(),
        )
    }

    #[test]
    fn returns_empty_when_no_changesets() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset_reader = MockChangesetReader::new();

        let operation = make_operation(project_provider, changeset_reader);

        let result = operation
            .execute(Path::new("/any"))
            .expect("StatusOperation failed for project with no changesets");

        assert!(result.changesets.is_empty());
        assert!(result.changeset_files.is_empty());
        assert!(result.projected_releases.is_empty());
        assert!(result.bumps_by_package.is_empty());
        assert_eq!(result.unchanged_packages.len(), 1);
        assert_eq!(result.unchanged_packages[0].name, "my-crate");
        assert!(result.packages_with_inherited_versions.is_empty());
        assert!(result.unknown_packages.is_empty());
    }

    #[test]
    fn collects_changesets_and_projected_releases() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");

        let changeset = make_changeset("my-crate", BumpType::Minor, "Add feature");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/test.md"), changeset);

        let operation = make_operation(project_provider, changeset_reader);

        let result = operation
            .execute(Path::new("/any"))
            .expect("StatusOperation failed to collect changesets");

        assert_eq!(result.changesets.len(), 1);
        assert_eq!(result.changeset_files.len(), 1);
        assert!(result.bumps_by_package.contains_key("my-crate"));
        assert_eq!(result.bumps_by_package["my-crate"], vec![BumpType::Minor]);
        assert!(result.unchanged_packages.is_empty());

        assert_eq!(result.projected_releases.len(), 1);
        let release = &result.projected_releases[0];
        assert_eq!(release.name, "my-crate");
        assert_eq!(release.current_version, Version::new(1, 0, 0));
        assert_eq!(release.new_version, Version::new(1, 1, 0));
        assert_eq!(release.bump_type, BumpType::Minor);
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

        let operation = make_operation(project_provider, changeset_reader);

        let result = operation
            .execute(Path::new("/any"))
            .expect("StatusOperation failed to aggregate multiple changesets");

        assert_eq!(result.changesets.len(), 2);
        assert_eq!(result.bumps_by_package["my-crate"].len(), 2);
        assert!(result.bumps_by_package["my-crate"].contains(&BumpType::Patch));
        assert!(result.bumps_by_package["my-crate"].contains(&BumpType::Minor));

        assert_eq!(result.projected_releases.len(), 1);
        let release = &result.projected_releases[0];
        assert_eq!(release.new_version, Version::new(1, 1, 0));
        assert_eq!(release.bump_type, BumpType::Minor);
    }

    #[test]
    fn identifies_unchanged_packages_in_workspace() {
        let project_provider =
            MockProjectProvider::workspace(vec![("crate-a", "1.0.0"), ("crate-b", "2.0.0")]);

        let changeset = make_changeset("crate-a", BumpType::Patch, "Fix crate-a");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/test.md"), changeset);

        let operation = make_operation(project_provider, changeset_reader);

        let result = operation
            .execute(Path::new("/any"))
            .expect("StatusOperation failed to identify unchanged packages");

        assert_eq!(result.unchanged_packages.len(), 1);
        assert_eq!(result.unchanged_packages[0].name, "crate-b");
    }

    #[test]
    fn detects_packages_with_inherited_versions() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset = make_changeset("my-crate", BumpType::Patch, "Fix");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/fix.md"), changeset);

        let inherited_checker = MockInheritedVersionChecker::new()
            .with_inherited(vec![PathBuf::from("/mock/project/Cargo.toml")]);

        let operation = StatusOperation::new(project_provider, changeset_reader, inherited_checker);

        let result = operation
            .execute(Path::new("/any"))
            .expect("StatusOperation failed to detect inherited versions");

        assert_eq!(result.packages_with_inherited_versions, vec!["my-crate"]);
    }

    #[test]
    fn collects_unknown_packages_as_warning() {
        let project_provider = MockProjectProvider::single_package("known-crate", "1.0.0");
        let changeset = make_changeset("unknown-crate", BumpType::Patch, "Fix");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/fix.md"), changeset);

        let operation = make_operation(project_provider, changeset_reader);

        let result = operation
            .execute(Path::new("/any"))
            .expect("StatusOperation failed to collect unknown packages");

        assert!(result.projected_releases.is_empty());
        assert_eq!(result.unknown_packages, vec!["unknown-crate"]);
    }

    #[test]
    fn projected_releases_match_version_planner_output() {
        let project_provider =
            MockProjectProvider::workspace(vec![("crate-a", "1.0.0"), ("crate-b", "2.5.3")]);

        let changeset1 = make_changeset("crate-a", BumpType::Minor, "Add feature");
        let changeset2 = make_changeset("crate-b", BumpType::Major, "Breaking change");

        let changeset_reader = MockChangesetReader::new().with_changesets(vec![
            (PathBuf::from(".changeset/feature.md"), changeset1),
            (PathBuf::from(".changeset/breaking.md"), changeset2),
        ]);

        let operation = make_operation(project_provider, changeset_reader);

        let result = operation
            .execute(Path::new("/any"))
            .expect("StatusOperation failed");

        assert_eq!(result.projected_releases.len(), 2);

        let release_a = result
            .projected_releases
            .iter()
            .find(|r| r.name == "crate-a")
            .expect("crate-a should be in releases");
        assert_eq!(release_a.current_version, Version::new(1, 0, 0));
        assert_eq!(release_a.new_version, Version::new(1, 1, 0));

        let release_b = result
            .projected_releases
            .iter()
            .find(|r| r.name == "crate-b")
            .expect("crate-b should be in releases");
        assert_eq!(release_b.current_version, Version::new(2, 5, 3));
        assert_eq!(release_b.new_version, Version::new(3, 0, 0));
    }

    #[test]
    fn propagates_inherited_version_checker_errors() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset_reader = MockChangesetReader::new();

        let operation = StatusOperation::new(
            project_provider,
            changeset_reader,
            FailingInheritedVersionChecker,
        );

        let result = operation.execute(Path::new("/any"));

        assert!(result.is_err());
    }

    #[test]
    fn returns_empty_consumed_changesets_when_none_exist() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset_reader = MockChangesetReader::new();

        let operation = make_operation(project_provider, changeset_reader);

        let result = operation
            .execute(Path::new("/any"))
            .expect("StatusOperation failed");

        assert!(result.consumed_prerelease_changesets.is_empty());
    }

    #[test]
    fn collects_consumed_prerelease_changesets() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");

        let mut consumed_changeset = make_changeset("my-crate", BumpType::Patch, "Fix bug");
        consumed_changeset.consumed_for_prerelease = Some("1.0.1-alpha.1".to_string());

        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/fix-bug.md"), consumed_changeset);

        let operation = make_operation(project_provider, changeset_reader);

        let result = operation
            .execute(Path::new("/any"))
            .expect("StatusOperation failed");

        assert!(result.changeset_files.is_empty());
        assert!(result.changesets.is_empty());
        assert_eq!(result.consumed_prerelease_changesets.len(), 1);
        assert_eq!(
            result.consumed_prerelease_changesets[0].0,
            PathBuf::from(".changeset/fix-bug.md")
        );
        assert_eq!(result.consumed_prerelease_changesets[0].1, "1.0.1-alpha.1");
    }

    #[test]
    fn separates_pending_and_consumed_changesets() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");

        let pending_changeset = make_changeset("my-crate", BumpType::Minor, "Add feature");

        let mut consumed_changeset = make_changeset("my-crate", BumpType::Patch, "Fix bug");
        consumed_changeset.consumed_for_prerelease = Some("1.0.1-alpha.1".to_string());

        let changeset_reader = MockChangesetReader::new().with_changesets(vec![
            (PathBuf::from(".changeset/feature.md"), pending_changeset),
            (PathBuf::from(".changeset/fix.md"), consumed_changeset),
        ]);

        let operation = make_operation(project_provider, changeset_reader);

        let result = operation
            .execute(Path::new("/any"))
            .expect("StatusOperation failed");

        assert_eq!(result.changeset_files.len(), 1);
        assert_eq!(
            result.changeset_files[0],
            PathBuf::from(".changeset/feature.md")
        );

        assert_eq!(result.changesets.len(), 1);
        assert_eq!(result.changesets[0].summary, "Add feature");

        assert_eq!(result.consumed_prerelease_changesets.len(), 1);
        assert_eq!(
            result.consumed_prerelease_changesets[0].0,
            PathBuf::from(".changeset/fix.md")
        );
        assert_eq!(result.consumed_prerelease_changesets[0].1, "1.0.1-alpha.1");
    }

    #[test]
    fn collects_multiple_consumed_changesets_with_different_versions() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");

        let mut consumed1 = make_changeset("my-crate", BumpType::Patch, "Fix bug 1");
        consumed1.consumed_for_prerelease = Some("1.0.1-alpha.1".to_string());

        let mut consumed2 = make_changeset("my-crate", BumpType::Patch, "Fix bug 2");
        consumed2.consumed_for_prerelease = Some("1.0.1-alpha.2".to_string());

        let changeset_reader = MockChangesetReader::new().with_changesets(vec![
            (PathBuf::from(".changeset/fix1.md"), consumed1),
            (PathBuf::from(".changeset/fix2.md"), consumed2),
        ]);

        let operation = make_operation(project_provider, changeset_reader);

        let result = operation
            .execute(Path::new("/any"))
            .expect("StatusOperation failed");

        assert!(result.changeset_files.is_empty());
        assert_eq!(result.consumed_prerelease_changesets.len(), 2);

        let versions: Vec<&str> = result
            .consumed_prerelease_changesets
            .iter()
            .map(|(_, v)| v.as_str())
            .collect();
        assert!(versions.contains(&"1.0.1-alpha.1"));
        assert!(versions.contains(&"1.0.1-alpha.2"));
    }
}
