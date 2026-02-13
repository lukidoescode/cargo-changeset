use std::path::{Path, PathBuf};

use changeset_changelog::{ChangelogLocation, ComparisonLinksSetting, RepositoryInfo};
use changeset_core::{BumpType, PackageInfo, PrereleaseSpec};
use changeset_project::{ProjectKind, TagFormat};
use chrono::Local;
use indexmap::IndexMap;
use semver::Version;

use super::changelog_aggregation::ChangesetAggregator;
use super::version_planner::VersionPlanner;
use crate::Result;
use crate::error::OperationError;
use crate::traits::{
    ChangelogWriter, ChangesetReader, ChangesetWriter, GitProvider, ManifestWriter, ProjectProvider,
};

#[allow(clippy::struct_excessive_bools)]
pub struct ReleaseInput {
    pub dry_run: bool,
    pub convert_inherited: bool,
    pub no_commit: bool,
    pub no_tags: bool,
    pub keep_changesets: bool,
    pub prerelease: Option<PrereleaseSpec>,
    pub force: bool,
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
pub struct CommitResult {
    pub sha: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct TagResult {
    pub name: String,
    pub target_sha: String,
}

#[derive(Debug, Clone, Default)]
pub struct GitOperationResult {
    pub commit: Option<CommitResult>,
    pub tags_created: Vec<TagResult>,
    pub changesets_deleted: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ReleaseOutput {
    pub planned_releases: Vec<PackageVersion>,
    pub unchanged_packages: Vec<String>,
    pub changesets_consumed: Vec<PathBuf>,
    pub changelog_updates: Vec<ChangelogUpdate>,
    pub git_result: Option<GitOperationResult>,
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

fn is_graduation_release(packages: &[PackageInfo], input: &ReleaseInput) -> bool {
    if input.prerelease.is_some() {
        return false;
    }
    packages
        .iter()
        .any(|p| changeset_version::is_prerelease(&p.version))
}

pub struct ReleaseOperation<P, RW, M, C, G> {
    project_provider: P,
    changeset_io: RW,
    manifest_writer: M,
    changelog_writer: C,
    git_provider: G,
}

impl<P, RW, M, C, G> ReleaseOperation<P, RW, M, C, G>
where
    P: ProjectProvider,
    RW: ChangesetReader + ChangesetWriter,
    M: ManifestWriter,
    C: ChangelogWriter,
    G: GitProvider,
{
    pub fn new(
        project_provider: P,
        changeset_io: RW,
        manifest_writer: M,
        changelog_writer: C,
        git_provider: G,
    ) -> Self {
        Self {
            project_provider,
            changeset_io,
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

    /// Validates that the working tree is clean when committing is enabled.
    ///
    /// # Errors
    ///
    /// Returns `OperationError::DirtyWorkingTree` if the working tree has uncommitted
    /// changes and committing is enabled.
    fn validate_working_tree(
        &self,
        project_root: &Path,
        should_commit: bool,
        dry_run: bool,
    ) -> Result<()> {
        if should_commit && !dry_run {
            let is_clean = self.git_provider.is_working_tree_clean(project_root)?;
            if !is_clean {
                return Err(OperationError::DirtyWorkingTree);
            }
        }
        Ok(())
    }

    /// Checks for packages with inherited versions and validates the convert flag.
    ///
    /// # Errors
    ///
    /// Returns `OperationError::InheritedVersionsRequireConvert` if packages use
    /// inherited versions and the convert flag is not set.
    fn check_inherited_versions(
        &self,
        packages: &[PackageInfo],
        convert_inherited: bool,
    ) -> Result<Vec<String>> {
        let inherited_packages = self.find_packages_with_inherited_versions(packages)?;
        if !inherited_packages.is_empty() && !convert_inherited {
            return Err(OperationError::InheritedVersionsRequireConvert {
                packages: inherited_packages,
            });
        }
        Ok(inherited_packages)
    }

    /// Loads changesets from the changeset directory and populates the aggregator.
    ///
    /// # Errors
    ///
    /// Returns an error if changeset files cannot be read or parsed.
    fn load_changesets(
        &self,
        changeset_dir: &Path,
        changeset_files: &[PathBuf],
    ) -> Result<(Vec<changeset_core::Changeset>, ChangesetAggregator)> {
        let mut changesets = Vec::new();
        let mut aggregator = ChangesetAggregator::new();

        for path in changeset_files {
            let changeset = self.changeset_io.read_changeset(path)?;
            aggregator.add_changeset(&changeset);
            changesets.push(changeset);
        }

        let consumed_paths = self.changeset_io.list_consumed_changesets(changeset_dir)?;
        for path in &consumed_paths {
            let changeset = self.changeset_io.read_changeset(path)?;
            aggregator.add_changeset(&changeset);
        }

        Ok((changesets, aggregator))
    }

    /// Handles marking changesets as consumed or clearing consumed flags.
    ///
    /// # Errors
    ///
    /// Returns an error if changeset files cannot be modified.
    fn handle_changeset_consumption(
        &self,
        changeset_dir: &Path,
        changeset_files: &[PathBuf],
        new_version: &Version,
        is_prerelease: bool,
        is_graduating: bool,
    ) -> Result<()> {
        if is_prerelease && !changeset_files.is_empty() {
            let paths_refs: Vec<&Path> = changeset_files.iter().map(AsRef::as_ref).collect();
            self.changeset_io.mark_consumed_for_prerelease(
                changeset_dir,
                &paths_refs,
                new_version,
            )?;
        }

        if is_graduating {
            let consumed_paths = self.changeset_io.list_consumed_changesets(changeset_dir)?;
            if !consumed_paths.is_empty() {
                let paths_refs: Vec<&Path> = consumed_paths.iter().map(AsRef::as_ref).collect();
                self.changeset_io
                    .clear_consumed_for_prerelease(changeset_dir, &paths_refs)?;
            }
        }

        Ok(())
    }

    /// Writes version updates to package manifests and optionally removes workspace version.
    ///
    /// # Errors
    ///
    /// Returns an error if manifest files cannot be written or verified.
    fn write_manifest_versions(
        &self,
        project_root: &Path,
        package_lookup: &IndexMap<String, PackageInfo>,
        planned_releases: &[PackageVersion],
        inherited_packages: &[String],
    ) -> Result<()> {
        if !inherited_packages.is_empty() {
            let root_manifest = project_root.join("Cargo.toml");
            self.manifest_writer
                .remove_workspace_version(&root_manifest)?;
        }

        for release in planned_releases {
            if let Some(pkg) = package_lookup.get(&release.name) {
                let manifest_path = pkg.path.join("Cargo.toml");
                self.manifest_writer
                    .write_version(&manifest_path, &release.new_version)?;
                self.manifest_writer
                    .verify_version(&manifest_path, &release.new_version)?;
            }
        }

        Ok(())
    }

    fn collect_unchanged_packages(
        packages: &[PackageInfo],
        planned_releases: &[PackageVersion],
    ) -> Vec<String> {
        let packages_with_releases: std::collections::HashSet<_> =
            planned_releases.iter().map(|r| r.name.clone()).collect();

        packages
            .iter()
            .filter(|p| !packages_with_releases.contains(&p.name))
            .map(|p| p.name.clone())
            .collect()
    }

    /// # Errors
    ///
    /// Returns an error if the project cannot be discovered, changeset files
    /// cannot be read, or manifest updates fail.
    pub fn execute(&self, start_path: &Path, input: &ReleaseInput) -> Result<ReleaseOutcome> {
        let project = self.project_provider.discover_project(start_path)?;
        let (root_config, _) = self.project_provider.load_configs(&project)?;

        let changeset_dir = project.root.join(root_config.changeset_dir());
        let changeset_files = self.changeset_io.list_changesets(&changeset_dir)?;
        let is_graduating = is_graduation_release(&project.packages, input);

        if changeset_files.is_empty() && !is_graduating {
            if input.prerelease.is_some() && !input.force {
                return Err(OperationError::NoChangesetsWithoutForce);
            }
            return Ok(ReleaseOutcome::NoChangesets);
        }

        let git_config = root_config.git_config();
        let should_commit = !input.no_commit && git_config.commit();
        let should_create_tags = !input.no_tags && git_config.tags();
        let should_delete_changesets = !input.keep_changesets && !git_config.keep_changesets();
        let is_prerelease_release = input.prerelease.is_some();

        self.validate_working_tree(&project.root, should_commit, input.dry_run)?;
        let inherited_packages =
            self.check_inherited_versions(&project.packages, input.convert_inherited)?;

        let (changesets, aggregator) = self.load_changesets(&changeset_dir, &changeset_files)?;

        let planned_releases = if is_graduating {
            VersionPlanner::plan_graduation(&project.packages)?.releases
        } else {
            VersionPlanner::plan_releases_with_prerelease(
                &changesets,
                &project.packages,
                input.prerelease.as_ref(),
            )?
            .releases
        };

        let package_lookup: IndexMap<_, _> = project
            .packages
            .iter()
            .map(|p| (p.name.clone(), p.clone()))
            .collect();

        let unchanged_packages =
            Self::collect_unchanged_packages(&project.packages, &planned_releases);

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
            changesets_consumed: changeset_files.clone(),
            changelog_updates,
            git_result: None,
        };

        if input.dry_run {
            return Ok(ReleaseOutcome::DryRun(output));
        }

        self.write_manifest_versions(
            &project.root,
            &package_lookup,
            &planned_releases,
            &inherited_packages,
        )?;

        if let Some(first_release) = planned_releases.first() {
            self.handle_changeset_consumption(
                &changeset_dir,
                &changeset_files,
                &first_release.new_version,
                is_prerelease_release,
                is_graduating,
            )?;
        }

        let should_delete_changesets_actual =
            should_delete_changesets && !is_prerelease_release && !is_graduating;

        let git_result = self.perform_git_operations(
            &project.root,
            &project.kind,
            &package_lookup,
            &planned_releases,
            &output.changelog_updates,
            &changeset_files,
            git_config,
            should_commit,
            should_create_tags,
            should_delete_changesets_actual,
            &inherited_packages,
        )?;

        Ok(ReleaseOutcome::Executed(ReleaseOutput {
            git_result: Some(git_result),
            ..output
        }))
    }

    #[allow(clippy::too_many_arguments)]
    fn perform_git_operations(
        &self,
        project_root: &Path,
        project_kind: &ProjectKind,
        package_lookup: &IndexMap<String, PackageInfo>,
        planned_releases: &[PackageVersion],
        changelog_updates: &[ChangelogUpdate],
        changeset_files: &[PathBuf],
        git_config: &changeset_project::GitConfig,
        should_commit: bool,
        should_create_tags: bool,
        should_delete_changesets: bool,
        inherited_packages: &[String],
    ) -> Result<GitOperationResult> {
        let mut result = GitOperationResult::default();

        let changesets_deleted = if should_delete_changesets {
            let paths_refs: Vec<&Path> = changeset_files.iter().map(AsRef::as_ref).collect();
            self.git_provider.delete_files(project_root, &paths_refs)?;
            changeset_files.to_vec()
        } else {
            Vec::new()
        };
        result.changesets_deleted = changesets_deleted;

        if !should_commit {
            return Ok(result);
        }

        let files_to_stage = Self::collect_files_to_stage(
            project_root,
            package_lookup,
            planned_releases,
            changelog_updates,
            &result.changesets_deleted,
            should_delete_changesets,
            inherited_packages,
        );

        let file_refs: Vec<&Path> = files_to_stage.iter().map(AsRef::as_ref).collect();
        self.git_provider.stage_files(project_root, &file_refs)?;

        let commit_message = Self::build_commit_message(planned_releases, git_config);
        let commit_info = self.git_provider.commit(project_root, &commit_message)?;
        result.commit = Some(CommitResult {
            sha: commit_info.sha,
            message: commit_info.message,
        });

        if should_create_tags {
            let tags = self.create_tags(
                project_root,
                project_kind,
                planned_releases,
                git_config.tag_format(),
            )?;
            result.tags_created = tags;
        }

        Ok(result)
    }

    fn collect_files_to_stage(
        project_root: &Path,
        package_lookup: &IndexMap<String, PackageInfo>,
        planned_releases: &[PackageVersion],
        changelog_updates: &[ChangelogUpdate],
        changesets_deleted: &[PathBuf],
        should_delete_changesets: bool,
        inherited_packages: &[String],
    ) -> Vec<PathBuf> {
        let mut files = Vec::new();

        for release in planned_releases {
            if let Some(pkg) = package_lookup.get(&release.name) {
                files.push(pkg.path.join("Cargo.toml"));
            }
        }

        if !inherited_packages.is_empty() {
            files.push(project_root.join("Cargo.toml"));
        }

        for update in changelog_updates {
            files.push(update.path.clone());
        }

        if should_delete_changesets {
            files.extend(changesets_deleted.iter().cloned());
        }

        files
    }

    fn build_commit_message(
        planned_releases: &[PackageVersion],
        git_config: &changeset_project::GitConfig,
    ) -> String {
        let version_list: Vec<String> = planned_releases
            .iter()
            .map(|r| format!("{}-v{}", r.name, r.new_version))
            .collect();
        let new_version = version_list.join(", ");

        let title = git_config
            .commit_title_template()
            .replace("{new-version}", &new_version);

        if !git_config.changes_in_body() {
            return title;
        }

        let body: Vec<String> = planned_releases
            .iter()
            .map(|r| format!("- {} {} -> {}", r.name, r.current_version, r.new_version))
            .collect();

        format!("{}\n\n{}", title, body.join("\n"))
    }

    fn create_tags(
        &self,
        project_root: &Path,
        project_kind: &ProjectKind,
        planned_releases: &[PackageVersion],
        config_tag_format: TagFormat,
    ) -> Result<Vec<TagResult>> {
        let use_crate_prefix = match project_kind {
            ProjectKind::SinglePackage => config_tag_format == TagFormat::CratePrefixed,
            ProjectKind::VirtualWorkspace | ProjectKind::WorkspaceWithRoot => true,
        };

        let mut tags = Vec::new();
        for release in planned_releases {
            let tag_name = if use_crate_prefix {
                format!("{}-v{}", release.name, release.new_version)
            } else {
                format!("v{}", release.new_version)
            };

            let tag_message = format!("Release {} v{}", release.name, release.new_version);

            let tag_info = self
                .git_provider
                .create_tag(project_root, &tag_name, &tag_message)?;

            tags.push(TagResult {
                name: tag_info.name,
                target_sha: tag_info.target_sha,
            });
        }

        Ok(tags)
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
            no_commit: true,
            no_tags: true,
            keep_changesets: true,
            prerelease: None,
            force: false,
        }
    }

    fn make_operation<P, RW, M>(
        project_provider: P,
        changeset_io: RW,
        manifest_writer: M,
    ) -> ReleaseOperation<P, RW, M, MockChangelogWriter, MockGitProvider>
    where
        P: ProjectProvider,
        RW: ChangesetReader + ChangesetWriter,
        M: ManifestWriter,
    {
        ReleaseOperation::new(
            project_provider,
            changeset_io,
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
            no_commit: true,
            no_tags: true,
            keep_changesets: true,
            prerelease: None,
            force: false,
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
            no_commit: true,
            no_tags: true,
            keep_changesets: true,
            prerelease: None,
            force: false,
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
            no_commit: true,
            no_tags: true,
            keep_changesets: true,
            prerelease: None,
            force: false,
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
            no_commit: true,
            no_tags: true,
            keep_changesets: true,
            prerelease: None,
            force: false,
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
            no_commit: true,
            no_tags: true,
            keep_changesets: true,
            prerelease: None,
            force: false,
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

    #[test]
    fn errors_on_dirty_working_tree_when_commit_enabled() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset = make_changeset("my-crate", BumpType::Patch, "Fix");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/fix.md"), changeset);
        let manifest_writer = MockManifestWriter::new();
        let git_provider = MockGitProvider::new().is_clean(false);

        let operation = ReleaseOperation::new(
            project_provider,
            changeset_reader,
            manifest_writer,
            MockChangelogWriter::new(),
            git_provider,
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: false,
            no_tags: true,
            keep_changesets: true,
            prerelease: None,
            force: false,
        };

        let result = operation.execute(Path::new("/any"), &input);

        assert!(matches!(result, Err(OperationError::DirtyWorkingTree)));
    }

    #[test]
    fn allows_dirty_tree_with_no_commit() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset = make_changeset("my-crate", BumpType::Patch, "Fix");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/fix.md"), changeset);
        let manifest_writer = MockManifestWriter::new();
        let git_provider = MockGitProvider::new().is_clean(false);

        let operation = ReleaseOperation::new(
            project_provider,
            changeset_reader,
            manifest_writer,
            MockChangelogWriter::new(),
            git_provider,
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: true,
            no_tags: true,
            keep_changesets: true,
            prerelease: None,
            force: false,
        };

        let result = operation.execute(Path::new("/any"), &input);

        assert!(result.is_ok());
    }

    #[test]
    fn allows_dirty_tree_in_dry_run() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset = make_changeset("my-crate", BumpType::Patch, "Fix");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/fix.md"), changeset);
        let manifest_writer = MockManifestWriter::new();
        let git_provider = MockGitProvider::new().is_clean(false);

        let operation = ReleaseOperation::new(
            project_provider,
            changeset_reader,
            manifest_writer,
            MockChangelogWriter::new(),
            git_provider,
        );
        let input = ReleaseInput {
            dry_run: true,
            convert_inherited: false,
            no_commit: false,
            no_tags: false,
            keep_changesets: false,
            prerelease: None,
            force: false,
        };

        let result = operation.execute(Path::new("/any"), &input);

        assert!(result.is_ok());
    }

    #[test]
    fn commit_message_uses_template() {
        use std::sync::Arc;

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset = make_changeset("my-crate", BumpType::Minor, "Add feature");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/feature.md"), changeset);
        let manifest_writer = MockManifestWriter::new();
        let git_provider = Arc::new(MockGitProvider::new());

        let operation = ReleaseOperation::new(
            project_provider,
            changeset_reader,
            manifest_writer,
            MockChangelogWriter::new(),
            Arc::clone(&git_provider),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: false,
            no_tags: true,
            keep_changesets: true,
            prerelease: None,
            force: false,
        };

        let ReleaseOutcome::Executed(output) = operation
            .execute(Path::new("/any"), &input)
            .expect("execute failed")
        else {
            panic!("expected Executed outcome");
        };

        let git_result = output.git_result.expect("should have git result");
        let commit = git_result.commit.expect("should have commit");
        assert!(commit.message.contains("my-crate-v1.1.0"));
        assert!(commit.message.contains("my-crate 1.0.0 -> 1.1.0"));
    }

    #[test]
    fn workspace_tags_use_crate_prefix() {
        use std::sync::Arc;

        let project_provider =
            MockProjectProvider::workspace(vec![("crate-a", "1.0.0"), ("crate-b", "2.0.0")]);
        let changeset1 = make_changeset("crate-a", BumpType::Patch, "Fix A");
        let changeset2 = make_changeset("crate-b", BumpType::Patch, "Fix B");
        let changeset_reader = MockChangesetReader::new().with_changesets(vec![
            (PathBuf::from(".changeset/fix-a.md"), changeset1),
            (PathBuf::from(".changeset/fix-b.md"), changeset2),
        ]);
        let manifest_writer = MockManifestWriter::new();
        let git_provider = Arc::new(MockGitProvider::new());

        let operation = ReleaseOperation::new(
            project_provider,
            changeset_reader,
            manifest_writer,
            MockChangelogWriter::new(),
            Arc::clone(&git_provider),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: false,
            no_tags: false,
            keep_changesets: true,
            prerelease: None,
            force: false,
        };

        let ReleaseOutcome::Executed(output) = operation
            .execute(Path::new("/any"), &input)
            .expect("execute failed")
        else {
            panic!("expected Executed outcome");
        };

        let git_result = output.git_result.expect("should have git result");
        assert_eq!(git_result.tags_created.len(), 2);

        let tag_names: Vec<_> = git_result.tags_created.iter().map(|t| &t.name).collect();
        assert!(tag_names.contains(&&"crate-a-v1.0.1".to_string()));
        assert!(tag_names.contains(&&"crate-b-v2.0.1".to_string()));
    }

    #[test]
    fn no_tags_skips_tag_creation() {
        use std::sync::Arc;

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset = make_changeset("my-crate", BumpType::Patch, "Fix");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/fix.md"), changeset);
        let manifest_writer = MockManifestWriter::new();
        let git_provider = Arc::new(MockGitProvider::new());

        let operation = ReleaseOperation::new(
            project_provider,
            changeset_reader,
            manifest_writer,
            MockChangelogWriter::new(),
            Arc::clone(&git_provider),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: false,
            no_tags: true,
            keep_changesets: true,
            prerelease: None,
            force: false,
        };

        let ReleaseOutcome::Executed(output) = operation
            .execute(Path::new("/any"), &input)
            .expect("execute failed")
        else {
            panic!("expected Executed outcome");
        };

        let git_result = output.git_result.expect("should have git result");
        assert!(git_result.tags_created.is_empty());
        assert!(git_result.commit.is_some());
    }

    #[test]
    fn single_package_uses_version_only_tag_format() {
        use std::sync::Arc;

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset = make_changeset("my-crate", BumpType::Patch, "Fix");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/fix.md"), changeset);
        let manifest_writer = MockManifestWriter::new();
        let git_provider = Arc::new(MockGitProvider::new());

        let operation = ReleaseOperation::new(
            project_provider,
            changeset_reader,
            manifest_writer,
            MockChangelogWriter::new(),
            Arc::clone(&git_provider),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: false,
            no_tags: false,
            keep_changesets: true,
            prerelease: None,
            force: false,
        };

        let ReleaseOutcome::Executed(output) = operation
            .execute(Path::new("/any"), &input)
            .expect("execute failed")
        else {
            panic!("expected Executed outcome");
        };

        let git_result = output.git_result.expect("should have git result");
        assert_eq!(git_result.tags_created.len(), 1);
        assert_eq!(
            git_result.tags_created[0].name, "v1.0.1",
            "single package should use version-only tag format without crate prefix"
        );
    }

    #[test]
    fn keep_changesets_false_populates_deleted_list() {
        use std::sync::Arc;

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset = make_changeset("my-crate", BumpType::Patch, "Fix");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/fix.md"), changeset);
        let manifest_writer = MockManifestWriter::new();
        let git_provider = Arc::new(MockGitProvider::new());

        let operation = ReleaseOperation::new(
            project_provider,
            changeset_reader,
            manifest_writer,
            MockChangelogWriter::new(),
            Arc::clone(&git_provider),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: true,
            no_tags: true,
            keep_changesets: false,
            prerelease: None,
            force: false,
        };

        let ReleaseOutcome::Executed(output) = operation
            .execute(Path::new("/any"), &input)
            .expect("execute failed")
        else {
            panic!("expected Executed outcome");
        };

        let git_result = output.git_result.expect("should have git result");
        assert_eq!(git_result.changesets_deleted.len(), 1);
        assert_eq!(
            git_result.changesets_deleted[0],
            PathBuf::from(".changeset/fix.md")
        );
    }

    #[test]
    fn keep_changesets_true_leaves_deleted_list_empty() {
        use std::sync::Arc;

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset = make_changeset("my-crate", BumpType::Patch, "Fix");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/fix.md"), changeset);
        let manifest_writer = MockManifestWriter::new();
        let git_provider = Arc::new(MockGitProvider::new());

        let operation = ReleaseOperation::new(
            project_provider,
            changeset_reader,
            manifest_writer,
            MockChangelogWriter::new(),
            Arc::clone(&git_provider),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: true,
            no_tags: true,
            keep_changesets: true,
            prerelease: None,
            force: false,
        };

        let ReleaseOutcome::Executed(output) = operation
            .execute(Path::new("/any"), &input)
            .expect("execute failed")
        else {
            panic!("expected Executed outcome");
        };

        let git_result = output.git_result.expect("should have git result");
        assert!(
            git_result.changesets_deleted.is_empty(),
            "changesets_deleted should be empty when keep_changesets is true"
        );
    }

    #[test]
    fn deleted_changesets_are_staged_for_commit() {
        use std::sync::Arc;

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset1 = make_changeset("my-crate", BumpType::Patch, "Fix 1");
        let changeset2 = make_changeset("my-crate", BumpType::Patch, "Fix 2");
        let changeset_reader = MockChangesetReader::new().with_changesets(vec![
            (PathBuf::from(".changeset/fix1.md"), changeset1),
            (PathBuf::from(".changeset/fix2.md"), changeset2),
        ]);
        let manifest_writer = MockManifestWriter::new();
        let git_provider = Arc::new(MockGitProvider::new());

        let operation = ReleaseOperation::new(
            project_provider,
            changeset_reader,
            manifest_writer,
            MockChangelogWriter::new(),
            Arc::clone(&git_provider),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: false,
            no_tags: true,
            keep_changesets: false,
            prerelease: None,
            force: false,
        };

        let _ = operation
            .execute(Path::new("/any"), &input)
            .expect("execute failed");

        let staged = git_provider.staged_files();
        assert!(
            staged.contains(&PathBuf::from(".changeset/fix1.md")),
            "fix1.md should be staged"
        );
        assert!(
            staged.contains(&PathBuf::from(".changeset/fix2.md")),
            "fix2.md should be staged"
        );
    }

    #[test]
    fn changes_in_body_false_produces_title_only_commit() {
        use changeset_project::{GitConfig, RootChangesetConfig};
        use std::sync::Arc;

        let custom_config = RootChangesetConfig::default()
            .with_git_config(GitConfig::default().with_changes_in_body(false));
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0")
            .with_root_config(custom_config);
        let changeset = make_changeset("my-crate", BumpType::Minor, "Add feature");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/feature.md"), changeset);
        let manifest_writer = MockManifestWriter::new();
        let git_provider = Arc::new(MockGitProvider::new());

        let operation = ReleaseOperation::new(
            project_provider,
            changeset_reader,
            manifest_writer,
            MockChangelogWriter::new(),
            Arc::clone(&git_provider),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: false,
            no_tags: true,
            keep_changesets: true,
            prerelease: None,
            force: false,
        };

        let ReleaseOutcome::Executed(output) = operation
            .execute(Path::new("/any"), &input)
            .expect("execute failed")
        else {
            panic!("expected Executed outcome");
        };

        let git_result = output.git_result.expect("should have git result");
        let commit = git_result.commit.expect("should have commit");
        assert!(
            !commit.message.contains('\n'),
            "commit message should be title-only without newlines, got: {}",
            commit.message
        );
        assert!(
            commit.message.contains("my-crate-v1.1.0"),
            "commit message should contain version info"
        );
    }

    #[test]
    fn test_prerelease_marks_changesets_as_consumed() {
        use std::sync::Arc;

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset_path = PathBuf::from(".changeset/fix.md");
        let changeset = make_changeset("my-crate", BumpType::Patch, "Fix bug");
        let changeset_reader =
            Arc::new(MockChangesetReader::new().with_changeset(changeset_path.clone(), changeset));
        let manifest_writer = MockManifestWriter::new();

        let operation = ReleaseOperation::new(
            project_provider,
            Arc::clone(&changeset_reader),
            manifest_writer,
            MockChangelogWriter::new(),
            MockGitProvider::new(),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: true,
            no_tags: true,
            keep_changesets: true,
            prerelease: Some(PrereleaseSpec::Alpha),
            force: false,
        };

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("execute should succeed");

        assert!(matches!(result, ReleaseOutcome::Executed(_)));

        let consumed_status = changeset_reader.get_consumed_status(&changeset_path);
        assert!(
            consumed_status.is_some(),
            "changeset should be marked as consumed for prerelease"
        );
        assert!(
            consumed_status.expect("checked above").contains("alpha"),
            "consumed version should contain alpha prerelease tag"
        );
    }

    #[test]
    fn test_prerelease_increment_requires_changesets_or_force() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset_reader = MockChangesetReader::new();
        let manifest_writer = MockManifestWriter::new();

        let operation = make_operation(project_provider, changeset_reader, manifest_writer);
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: true,
            no_tags: true,
            keep_changesets: true,
            prerelease: Some(PrereleaseSpec::Alpha),
            force: false,
        };

        let result = operation.execute(Path::new("/any"), &input);

        assert!(
            matches!(result, Err(OperationError::NoChangesetsWithoutForce)),
            "should error without changesets and without force flag"
        );
    }

    #[test]
    fn test_prerelease_with_force_returns_no_changesets() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset_reader = MockChangesetReader::new();
        let manifest_writer = MockManifestWriter::new();

        let operation = make_operation(project_provider, changeset_reader, manifest_writer);
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: true,
            no_tags: true,
            keep_changesets: true,
            prerelease: Some(PrereleaseSpec::Alpha),
            force: true,
        };

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("execute should succeed with force flag");

        assert!(
            matches!(result, ReleaseOutcome::NoChangesets),
            "should return NoChangesets when force is set but no changesets exist"
        );
    }

    #[test]
    fn test_graduation_clears_consumed_flag() {
        use std::sync::Arc;

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.1-alpha.1");
        let consumed_path = PathBuf::from(".changeset/consumed.md");
        let changeset = make_changeset("my-crate", BumpType::Patch, "Fix bug");
        let changeset_reader = Arc::new(MockChangesetReader::new().with_consumed_changeset(
            consumed_path.clone(),
            changeset,
            "1.0.1-alpha.1".to_string(),
        ));
        let manifest_writer = MockManifestWriter::new();

        let operation = ReleaseOperation::new(
            project_provider,
            Arc::clone(&changeset_reader),
            manifest_writer,
            MockChangelogWriter::new(),
            MockGitProvider::new(),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: true,
            no_tags: true,
            keep_changesets: true,
            prerelease: None,
            force: false,
        };

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("graduation should succeed");

        assert!(matches!(result, ReleaseOutcome::Executed(_)));

        let consumed_status = changeset_reader.get_consumed_status(&consumed_path);
        assert!(
            consumed_status.is_none(),
            "consumed flag should be cleared after graduation"
        );
    }

    #[test]
    fn test_graduation_aggregates_consumed_changesets_in_changelog() {
        use std::sync::Arc;

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.1-alpha.1");
        let consumed_path1 = PathBuf::from(".changeset/fix1.md");
        let consumed_path2 = PathBuf::from(".changeset/fix2.md");
        let changeset1 = make_changeset("my-crate", BumpType::Patch, "Fix bug one");
        let changeset2 = make_changeset("my-crate", BumpType::Patch, "Fix bug two");

        let changeset_reader = Arc::new(
            MockChangesetReader::new()
                .with_consumed_changeset(consumed_path1, changeset1, "1.0.1-alpha.1".to_string())
                .with_consumed_changeset(consumed_path2, changeset2, "1.0.1-alpha.1".to_string()),
        );
        let manifest_writer = MockManifestWriter::new();
        let changelog_writer = Arc::new(MockChangelogWriter::new());

        let operation = ReleaseOperation::new(
            project_provider,
            Arc::clone(&changeset_reader),
            manifest_writer,
            Arc::clone(&changelog_writer),
            MockGitProvider::new(),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: true,
            no_tags: true,
            keep_changesets: true,
            prerelease: None,
            force: false,
        };

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("graduation should succeed");

        assert!(matches!(result, ReleaseOutcome::Executed(_)));

        let written = changelog_writer.written_releases();
        assert_eq!(written.len(), 1, "should write one changelog release");

        let (_, release) = &written[0];
        assert_eq!(
            release.entries.len(),
            2,
            "changelog should contain entries from both consumed changesets"
        );
    }

    #[test]
    fn test_consumed_changesets_excluded_from_normal_release() {
        use std::sync::Arc;

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let unconsumed_path = PathBuf::from(".changeset/unconsumed.md");
        let consumed_path = PathBuf::from(".changeset/consumed.md");
        let unconsumed_changeset = make_changeset("my-crate", BumpType::Minor, "Add feature");
        let consumed_changeset = make_changeset("my-crate", BumpType::Patch, "Fix from prerelease");

        let changeset_reader = Arc::new(
            MockChangesetReader::new()
                .with_changeset(unconsumed_path.clone(), unconsumed_changeset)
                .with_consumed_changeset(
                    consumed_path.clone(),
                    consumed_changeset,
                    "1.0.1-alpha.1".to_string(),
                ),
        );
        let manifest_writer = Arc::new(MockManifestWriter::new());

        let operation = ReleaseOperation::new(
            project_provider,
            Arc::clone(&changeset_reader),
            Arc::clone(&manifest_writer),
            MockChangelogWriter::new(),
            MockGitProvider::new(),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: true,
            no_tags: true,
            keep_changesets: true,
            prerelease: None,
            force: false,
        };

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("release should succeed");

        let ReleaseOutcome::Executed(output) = result else {
            panic!("expected Executed outcome");
        };

        assert_eq!(output.planned_releases.len(), 1);
        assert_eq!(
            output.planned_releases[0].new_version.to_string(),
            "1.1.0",
            "should apply minor bump from unconsumed changeset only"
        );

        assert_eq!(
            output.changesets_consumed.len(),
            1,
            "only unconsumed changeset should be in consumed list"
        );
        assert!(
            output.changesets_consumed.contains(&unconsumed_path),
            "unconsumed changeset should be processed"
        );
    }

    #[test]
    fn test_prerelease_with_different_tag_resets_number() {
        use std::sync::Arc;

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.1-alpha.2");
        let changeset_path = PathBuf::from(".changeset/feature.md");
        let changeset = make_changeset("my-crate", BumpType::Patch, "Another fix");
        let changeset_reader =
            Arc::new(MockChangesetReader::new().with_changeset(changeset_path, changeset));
        let manifest_writer = Arc::new(MockManifestWriter::new());

        let operation = ReleaseOperation::new(
            project_provider,
            Arc::clone(&changeset_reader),
            Arc::clone(&manifest_writer),
            MockChangelogWriter::new(),
            MockGitProvider::new(),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: true,
            no_tags: true,
            keep_changesets: true,
            prerelease: Some(PrereleaseSpec::Beta),
            force: false,
        };

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("prerelease with different tag should succeed");

        let ReleaseOutcome::Executed(output) = result else {
            panic!("expected Executed outcome");
        };

        assert_eq!(output.planned_releases.len(), 1);
        assert_eq!(
            output.planned_releases[0].new_version.to_string(),
            "1.0.1-beta.1",
            "switching prerelease tag should reset number to 1"
        );
    }
}
