use std::path::{Path, PathBuf};

use changeset_changelog::{ChangelogLocation, ComparisonLinksSetting, RepositoryInfo};
use changeset_core::{BumpType, PackageInfo};
use chrono::Local;
use indexmap::IndexMap;
use semver::Version;

use super::changelog_aggregation::ChangesetAggregator;
use super::version_planner::VersionPlanner;
use crate::Result;
use crate::error::OperationError;
use crate::traits::{
    ChangelogWriter, ChangesetReader, GitProvider, ManifestWriter, ProjectProvider,
};

pub struct ReleaseInput {
    pub dry_run: bool,
    pub convert_inherited: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageVersion {
    pub name: String,
    pub current_version: Version,
    pub new_version: Version,
    pub bump_type: BumpType,
}

#[derive(Debug, Clone)]
pub struct ChangelogUpdate {
    pub path: PathBuf,
    pub package: Option<String>,
    pub version: Version,
    pub created: bool,
}

#[derive(Debug, Clone)]
pub struct ReleaseOutput {
    pub planned_releases: Vec<PackageVersion>,
    pub unchanged_packages: Vec<String>,
    pub changesets_consumed: Vec<PathBuf>,
    pub changelog_updates: Vec<ChangelogUpdate>,
}

#[derive(Debug)]
pub enum ReleaseOutcome {
    DryRun(ReleaseOutput),
    Executed(ReleaseOutput),
    NoChangesets,
}

fn find_previous_tag(planned_releases: &[PackageVersion]) -> Option<String> {
    let first_release = planned_releases.first()?;
    let previous_version = &first_release.current_version;
    Some(previous_version.to_string())
}

pub struct ReleaseOperation<P, R, M, C, G> {
    project_provider: P,
    changeset_reader: R,
    manifest_writer: M,
    changelog_writer: C,
    git_provider: G,
}

impl<P, R, M, C, G> ReleaseOperation<P, R, M, C, G>
where
    P: ProjectProvider,
    R: ChangesetReader,
    M: ManifestWriter,
    C: ChangelogWriter,
    G: GitProvider,
{
    pub fn new(
        project_provider: P,
        changeset_reader: R,
        manifest_writer: M,
        changelog_writer: C,
        git_provider: G,
    ) -> Self {
        Self {
            project_provider,
            changeset_reader,
            manifest_writer,
            changelog_writer,
            git_provider,
        }
    }

    fn find_packages_with_inherited_versions(
        &self,
        packages: &[PackageInfo],
    ) -> Result<Vec<String>> {
        self.manifest_writer
            .find_packages_with_inherited_versions(packages)
    }

    fn detect_repository_info(&self, project_root: &Path) -> Option<RepositoryInfo> {
        let url = self.git_provider.remote_url(project_root).ok()??;
        RepositoryInfo::from_url(&url).ok()
    }

    fn generate_changelog_updates(
        &self,
        project_root: &Path,
        changelog_config: &changeset_changelog::ChangelogConfig,
        aggregator: &ChangesetAggregator,
        planned_releases: &[PackageVersion],
        package_lookup: &IndexMap<String, PackageInfo>,
    ) -> Result<Vec<ChangelogUpdate>> {
        let today = Local::now().date_naive();
        let repo_info = self.resolve_repo_info(project_root, changelog_config)?;
        let mut changelog_updates = Vec::new();

        match changelog_config.changelog {
            ChangelogLocation::Root => {
                let changelog_path = project_root.join("CHANGELOG.md");
                let max_version = planned_releases
                    .iter()
                    .map(|r| &r.new_version)
                    .max()
                    .cloned();

                if let Some(version) = max_version {
                    let packages: Vec<_> = planned_releases
                        .iter()
                        .map(|r| (r.name.clone(), r.new_version.clone()))
                        .collect();

                    if let Some(release) = aggregator.build_root_release(&version, today, &packages)
                    {
                        let previous_tag = find_previous_tag(planned_releases);

                        let result = self.changelog_writer.write_release(
                            &changelog_path,
                            &release,
                            repo_info.as_ref(),
                            previous_tag.as_deref(),
                        )?;

                        changelog_updates.push(ChangelogUpdate {
                            path: result.path,
                            package: None,
                            version,
                            created: result.created,
                        });
                    }
                }
            }
            ChangelogLocation::PerPackage => {
                for release in planned_releases {
                    if let Some(pkg) = package_lookup.get(&release.name) {
                        let changelog_path = pkg.path.join("CHANGELOG.md");

                        if let Some(version_release) = aggregator.build_package_release(
                            &release.name,
                            &release.new_version,
                            today,
                        ) {
                            let previous_version = release.current_version.to_string();

                            let result = self.changelog_writer.write_release(
                                &changelog_path,
                                &version_release,
                                repo_info.as_ref(),
                                Some(&previous_version),
                            )?;

                            changelog_updates.push(ChangelogUpdate {
                                path: result.path,
                                package: Some(release.name.clone()),
                                version: release.new_version.clone(),
                                created: result.created,
                            });
                        }
                    }
                }
            }
        }

        Ok(changelog_updates)
    }

    fn resolve_repo_info(
        &self,
        project_root: &Path,
        changelog_config: &changeset_changelog::ChangelogConfig,
    ) -> Result<Option<RepositoryInfo>> {
        match changelog_config.comparison_links {
            ComparisonLinksSetting::Disabled => Ok(None),
            ComparisonLinksSetting::Auto => Ok(self.detect_repository_info(project_root)),
            ComparisonLinksSetting::Enabled => {
                let repo_info = self.detect_repository_info(project_root);
                if repo_info.is_none() {
                    return Err(OperationError::ComparisonLinksRequired);
                }
                Ok(repo_info)
            }
        }
    }

    /// # Errors
    ///
    /// Returns an error if the project cannot be discovered, changeset files
    /// cannot be read, or manifest updates fail.
    pub fn execute(&self, start_path: &Path, input: &ReleaseInput) -> Result<ReleaseOutcome> {
        let project = self.project_provider.discover_project(start_path)?;
        let (root_config, _) = self.project_provider.load_configs(&project)?;

        let changeset_dir = project.root.join(root_config.changeset_dir());
        let changeset_files = self.changeset_reader.list_changesets(&changeset_dir)?;

        if changeset_files.is_empty() {
            return Ok(ReleaseOutcome::NoChangesets);
        }

        let inherited_packages = self.find_packages_with_inherited_versions(&project.packages)?;
        if !inherited_packages.is_empty() && !input.convert_inherited {
            return Err(OperationError::InheritedVersionsRequireConvert {
                packages: inherited_packages,
            });
        }

        let mut changesets = Vec::new();
        let mut aggregator = ChangesetAggregator::new();

        for path in &changeset_files {
            let changeset = self.changeset_reader.read_changeset(path)?;
            aggregator.add_changeset(&changeset);
            changesets.push(changeset);
        }

        let plan = VersionPlanner::plan_releases(&changesets, &project.packages);
        let planned_releases = plan.releases;

        let package_lookup: IndexMap<_, _> = project
            .packages
            .iter()
            .map(|p| (p.name.clone(), p.clone()))
            .collect();

        let packages_with_releases: std::collections::HashSet<_> =
            planned_releases.iter().map(|r| r.name.clone()).collect();

        let unchanged_packages: Vec<String> = project
            .packages
            .iter()
            .filter(|p| !packages_with_releases.contains(&p.name))
            .map(|p| p.name.clone())
            .collect();

        let changelog_updates = if input.dry_run {
            Vec::new()
        } else {
            self.generate_changelog_updates(
                &project.root,
                root_config.changelog_config(),
                &aggregator,
                &planned_releases,
                &package_lookup,
            )?
        };

        let output = ReleaseOutput {
            planned_releases: planned_releases.clone(),
            unchanged_packages,
            changesets_consumed: changeset_files,
            changelog_updates,
        };

        if input.dry_run {
            return Ok(ReleaseOutcome::DryRun(output));
        }

        if !inherited_packages.is_empty() {
            let root_manifest = project.root.join("Cargo.toml");
            self.manifest_writer
                .remove_workspace_version(&root_manifest)?;
        }

        for release in &planned_releases {
            if let Some(pkg) = package_lookup.get(&release.name) {
                let manifest_path = pkg.path.join("Cargo.toml");
                self.manifest_writer
                    .write_version(&manifest_path, &release.new_version)?;
                self.manifest_writer
                    .verify_version(&manifest_path, &release.new_version)?;
            }
        }

        Ok(ReleaseOutcome::Executed(output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mocks::{
        MockChangelogWriter, MockChangesetReader, MockGitProvider, MockManifestWriter,
        MockProjectProvider, make_changeset,
    };

    fn default_input() -> ReleaseInput {
        ReleaseInput {
            dry_run: true,
            convert_inherited: false,
        }
    }

    fn make_operation<P, R, M>(
        project_provider: P,
        changeset_reader: R,
        manifest_writer: M,
    ) -> ReleaseOperation<P, R, M, MockChangelogWriter, MockGitProvider>
    where
        P: ProjectProvider,
        R: ChangesetReader,
        M: ManifestWriter,
    {
        ReleaseOperation::new(
            project_provider,
            changeset_reader,
            manifest_writer,
            MockChangelogWriter::new(),
            MockGitProvider::new(),
        )
    }

    #[test]
    fn returns_no_changesets_when_empty() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset_reader = MockChangesetReader::new();
        let manifest_writer = MockManifestWriter::new();

        let operation = make_operation(project_provider, changeset_reader, manifest_writer);

        let result = operation
            .execute(Path::new("/any"), &default_input())
            .expect("execute failed");

        assert!(matches!(result, ReleaseOutcome::NoChangesets));
    }

    #[test]
    fn calculates_single_patch_bump() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset = make_changeset("my-crate", BumpType::Patch, "Fix a bug");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/fix.md"), changeset);
        let manifest_writer = MockManifestWriter::new();

        let operation = make_operation(project_provider, changeset_reader, manifest_writer);

        let result = operation
            .execute(Path::new("/any"), &default_input())
            .expect("execute failed");

        let ReleaseOutcome::DryRun(output) = result else {
            panic!("expected DryRun outcome");
        };

        assert_eq!(output.planned_releases.len(), 1);
        let release = &output.planned_releases[0];
        assert_eq!(release.name, "my-crate");
        assert_eq!(release.current_version.to_string(), "1.0.0");
        assert_eq!(release.new_version.to_string(), "1.0.1");
        assert_eq!(release.bump_type, BumpType::Patch);
    }

    #[test]
    fn takes_maximum_bump_from_multiple_changesets() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.2.3");
        let changeset1 = make_changeset("my-crate", BumpType::Patch, "Fix bug");
        let changeset2 = make_changeset("my-crate", BumpType::Minor, "Add feature");

        let changeset_reader = MockChangesetReader::new().with_changesets(vec![
            (PathBuf::from(".changeset/fix.md"), changeset1),
            (PathBuf::from(".changeset/feature.md"), changeset2),
        ]);
        let manifest_writer = MockManifestWriter::new();

        let operation = make_operation(project_provider, changeset_reader, manifest_writer);

        let result = operation
            .execute(Path::new("/any"), &default_input())
            .expect("execute failed");

        let ReleaseOutcome::DryRun(output) = result else {
            panic!("expected DryRun outcome");
        };

        assert_eq!(output.planned_releases.len(), 1);
        let release = &output.planned_releases[0];
        assert_eq!(release.new_version.to_string(), "1.3.0");
        assert_eq!(release.bump_type, BumpType::Minor);
    }

    #[test]
    fn handles_workspace_with_multiple_packages() {
        let project_provider =
            MockProjectProvider::workspace(vec![("crate-a", "1.0.0"), ("crate-b", "2.0.0")]);

        let changeset1 = make_changeset("crate-a", BumpType::Minor, "Add feature to A");
        let changeset2 = make_changeset("crate-b", BumpType::Major, "Breaking change in B");

        let changeset_reader = MockChangesetReader::new().with_changesets(vec![
            (PathBuf::from(".changeset/feature-a.md"), changeset1),
            (PathBuf::from(".changeset/breaking-b.md"), changeset2),
        ]);
        let manifest_writer = MockManifestWriter::new();

        let operation = make_operation(project_provider, changeset_reader, manifest_writer);

        let result = operation
            .execute(Path::new("/any"), &default_input())
            .expect("execute failed");

        let ReleaseOutcome::DryRun(output) = result else {
            panic!("expected DryRun outcome");
        };

        assert_eq!(output.planned_releases.len(), 2);
        assert!(output.unchanged_packages.is_empty());

        let crate_a = output
            .planned_releases
            .iter()
            .find(|r| r.name == "crate-a")
            .expect("crate-a should be in releases");
        assert_eq!(crate_a.new_version.to_string(), "1.1.0");

        let crate_b = output
            .planned_releases
            .iter()
            .find(|r| r.name == "crate-b")
            .expect("crate-b should be in releases");
        assert_eq!(crate_b.new_version.to_string(), "3.0.0");
    }

    #[test]
    fn identifies_unchanged_packages() {
        let project_provider = MockProjectProvider::workspace(vec![
            ("crate-a", "1.0.0"),
            ("crate-b", "2.0.0"),
            ("crate-c", "3.0.0"),
        ]);

        let changeset = make_changeset("crate-a", BumpType::Patch, "Fix crate-a");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/fix.md"), changeset);
        let manifest_writer = MockManifestWriter::new();

        let operation = make_operation(project_provider, changeset_reader, manifest_writer);

        let result = operation
            .execute(Path::new("/any"), &default_input())
            .expect("execute failed");

        let ReleaseOutcome::DryRun(output) = result else {
            panic!("expected DryRun outcome");
        };

        assert_eq!(output.planned_releases.len(), 1);
        assert_eq!(output.unchanged_packages.len(), 2);
        assert!(output.unchanged_packages.contains(&"crate-b".to_string()));
        assert!(output.unchanged_packages.contains(&"crate-c".to_string()));
    }

    #[test]
    fn tracks_consumed_changeset_files() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset1 = make_changeset("my-crate", BumpType::Patch, "Fix 1");
        let changeset2 = make_changeset("my-crate", BumpType::Patch, "Fix 2");

        let changeset_reader = MockChangesetReader::new().with_changesets(vec![
            (PathBuf::from(".changeset/fix1.md"), changeset1),
            (PathBuf::from(".changeset/fix2.md"), changeset2),
        ]);
        let manifest_writer = MockManifestWriter::new();

        let operation = make_operation(project_provider, changeset_reader, manifest_writer);

        let result = operation
            .execute(Path::new("/any"), &default_input())
            .expect("execute failed");

        let ReleaseOutcome::DryRun(output) = result else {
            panic!("expected DryRun outcome");
        };

        assert_eq!(output.changesets_consumed.len(), 2);
    }

    #[test]
    fn returns_executed_when_not_dry_run() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset = make_changeset("my-crate", BumpType::Patch, "Fix");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/fix.md"), changeset);
        let manifest_writer = MockManifestWriter::new();

        let operation = make_operation(project_provider, changeset_reader, manifest_writer);
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
        };

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("execute failed");

        assert!(matches!(result, ReleaseOutcome::Executed(_)));
    }

    #[test]
    fn writes_versions_when_not_dry_run() {
        use std::sync::Arc;

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset = make_changeset("my-crate", BumpType::Minor, "Add feature");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/feature.md"), changeset);
        let manifest_writer = Arc::new(MockManifestWriter::new());

        let operation = ReleaseOperation::new(
            project_provider,
            changeset_reader,
            Arc::clone(&manifest_writer),
            MockChangelogWriter::new(),
            MockGitProvider::new(),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
        };

        let ReleaseOutcome::Executed(output) = operation
            .execute(Path::new("/any"), &input)
            .expect("execute failed")
        else {
            panic!("expected Executed outcome");
        };

        assert_eq!(output.planned_releases.len(), 1);
        assert_eq!(output.planned_releases[0].new_version.to_string(), "1.1.0");

        let written = manifest_writer.written_versions();
        assert_eq!(written.len(), 1);
        assert_eq!(written[0].0, PathBuf::from("/mock/project/Cargo.toml"));
        assert_eq!(written[0].1.to_string(), "1.1.0");
    }

    #[test]
    fn returns_error_when_inherited_without_convert_flag() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset = make_changeset("my-crate", BumpType::Patch, "Fix");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/fix.md"), changeset);
        let manifest_writer = MockManifestWriter::new()
            .with_inherited(vec![PathBuf::from("/mock/project/Cargo.toml")]);

        let operation = make_operation(project_provider, changeset_reader, manifest_writer);
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
        };

        let result = operation.execute(Path::new("/any"), &input);

        assert!(matches!(
            result,
            Err(OperationError::InheritedVersionsRequireConvert { .. })
        ));
    }

    #[test]
    fn allows_inherited_with_convert_flag() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset = make_changeset("my-crate", BumpType::Patch, "Fix");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/fix.md"), changeset);
        let manifest_writer = MockManifestWriter::new()
            .with_inherited(vec![PathBuf::from("/mock/project/Cargo.toml")]);

        let operation = make_operation(project_provider, changeset_reader, manifest_writer);
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: true,
        };

        let result = operation.execute(Path::new("/any"), &input);

        assert!(result.is_ok());
    }

    #[test]
    fn removes_workspace_version_when_converting_inherited() {
        use std::sync::Arc;

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset = make_changeset("my-crate", BumpType::Patch, "Fix");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/fix.md"), changeset);
        let manifest_writer = Arc::new(
            MockManifestWriter::new()
                .with_inherited(vec![PathBuf::from("/mock/project/Cargo.toml")]),
        );

        let operation = ReleaseOperation::new(
            project_provider,
            changeset_reader,
            Arc::clone(&manifest_writer),
            MockChangelogWriter::new(),
            MockGitProvider::new(),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: true,
        };

        let ReleaseOutcome::Executed(_) = operation
            .execute(Path::new("/any"), &input)
            .expect("execute failed")
        else {
            panic!("expected Executed outcome");
        };

        assert!(
            manifest_writer.workspace_version_removed(),
            "workspace version should be removed"
        );
    }
}
