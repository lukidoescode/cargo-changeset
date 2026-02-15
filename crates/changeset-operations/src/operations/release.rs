use std::collections::HashMap;
use std::path::{Path, PathBuf};

use changeset_changelog::{ChangelogLocation, ComparisonLinksSetting, RepositoryInfo};
use changeset_core::{BumpType, PackageInfo, PrereleaseSpec};
use changeset_project::{GraduationState, ProjectKind, TagFormat};
use chrono::Local;
use indexmap::IndexMap;
use semver::Version;

use super::changelog_aggregation::ChangesetAggregator;
use super::release_validator::{PackageReleaseConfig, ReleaseCliInput, ReleaseValidator};
use super::version_planner::VersionPlanner;
use crate::Result;
use crate::error::OperationError;
use crate::traits::{
    ChangelogWriter, ChangesetReader, ChangesetWriter, GitProvider, ManifestWriter,
    ProjectProvider, ReleaseStateIO,
};

pub struct ReleaseInput {
    pub dry_run: bool,
    pub convert_inherited: bool,
    pub no_commit: bool,
    pub no_tags: bool,
    pub keep_changesets: bool,
    pub force: bool,
    /// Per-package release configuration from CLI (merged with TOML state at execution).
    pub per_package_config: HashMap<String, PackageReleaseConfig>,
    /// Global prerelease tag (applies to all packages without specific config).
    pub global_prerelease: Option<PrereleaseSpec>,
    /// Whether `--graduate` was passed without specific crates (single-package mode).
    pub graduate_all: bool,
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

struct GitOptions {
    should_commit: bool,
    should_create_tags: bool,
    should_delete_changesets: bool,
}

struct ReleaseContext {
    project: changeset_project::CargoProject,
    root_config: changeset_project::RootChangesetConfig,
    changeset_dir: PathBuf,
    changeset_files: Vec<PathBuf>,
    prerelease_state: Option<changeset_project::PrereleaseState>,
    graduation_state: Option<GraduationState>,
    per_package_config: HashMap<String, PackageReleaseConfig>,
    is_prerelease_graduation: bool,
    is_graduating: bool,
    is_prerelease_release: bool,
    git_options: GitOptions,
    inherited_packages: Vec<String>,
    early_return: Option<Result<ReleaseOutcome>>,
}

struct ReleasePlan {
    output: ReleaseOutput,
    planned_releases: Vec<PackageVersion>,
    package_lookup: IndexMap<String, PackageInfo>,
}

fn find_previous_tag(planned_releases: &[PackageVersion]) -> Option<String> {
    let first_release = planned_releases.first()?;
    let previous_version = &first_release.current_version;
    Some(previous_version.to_string())
}

fn is_any_prerelease_configured(
    input: &ReleaseInput,
    per_package_config: &HashMap<String, PackageReleaseConfig>,
) -> bool {
    input.global_prerelease.is_some() || per_package_config.values().any(|c| c.prerelease.is_some())
}

fn is_prerelease_graduation(
    packages: &[PackageInfo],
    per_package_config: &HashMap<String, PackageReleaseConfig>,
) -> bool {
    if per_package_config.values().any(|c| c.prerelease.is_some()) {
        return false;
    }
    packages
        .iter()
        .any(|p| changeset_version::is_prerelease(&p.version))
}

fn is_zero_graduation(
    packages: &[PackageInfo],
    input: &ReleaseInput,
    per_package_config: &HashMap<String, PackageReleaseConfig>,
) -> bool {
    let has_graduation = input.graduate_all || per_package_config.values().any(|c| c.graduate_zero);
    if !has_graduation {
        return false;
    }
    packages
        .iter()
        .any(|p| changeset_version::is_zero_version(&p.version))
}

pub struct ReleaseOperation<P, RW, M, C, G, S> {
    project_provider: P,
    changeset_io: RW,
    manifest_writer: M,
    changelog_writer: C,
    git_provider: G,
    release_state_io: S,
}

impl<P, RW, M, C, G, S> ReleaseOperation<P, RW, M, C, G, S>
where
    P: ProjectProvider,
    RW: ChangesetReader + ChangesetWriter,
    M: ManifestWriter,
    C: ChangelogWriter,
    G: GitProvider,
    S: ReleaseStateIO,
{
    pub fn new(
        project_provider: P,
        changeset_io: RW,
        manifest_writer: M,
        changelog_writer: C,
        git_provider: G,
        release_state_io: S,
    ) -> Self {
        Self {
            project_provider,
            changeset_io,
            manifest_writer,
            changelog_writer,
            git_provider,
            release_state_io,
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
        let context = self.prepare_release_context(start_path, input)?;

        if let Some(early_return) = context.early_return {
            return early_return;
        }

        let plan = self.plan_release(&context, input.dry_run)?;

        if input.dry_run {
            return Ok(ReleaseOutcome::DryRun(plan.output));
        }

        self.execute_release(&context, plan)
    }

    fn prepare_release_context(
        &self,
        start_path: &Path,
        input: &ReleaseInput,
    ) -> Result<ReleaseContext> {
        let project = self.project_provider.discover_project(start_path)?;
        let (root_config, _) = self.project_provider.load_configs(&project)?;

        let changeset_dir = project.root.join(root_config.changeset_dir());
        let changeset_files = self.changeset_io.list_changesets(&changeset_dir)?;

        let prerelease_state = self
            .release_state_io
            .load_prerelease_state(&changeset_dir)?;
        let graduation_state = self
            .release_state_io
            .load_graduation_state(&changeset_dir)?;

        let cli_input = Self::build_cli_input(input);
        let validated_config = ReleaseValidator::validate(
            &cli_input,
            prerelease_state.as_ref(),
            graduation_state.as_ref(),
            &project.packages,
            &project.kind,
        )
        .map_err(OperationError::ValidationFailed)?;

        let per_package_config = validated_config.per_package;

        let is_prerelease_graduation =
            is_prerelease_graduation(&project.packages, &per_package_config);
        let is_zero_graduation = is_zero_graduation(&project.packages, input, &per_package_config);
        let is_graduating = is_prerelease_graduation || is_zero_graduation;

        let early_return =
            Self::check_early_return(&changeset_files, is_graduating, input, &per_package_config);

        let git_config = root_config.git_config();
        let git_options = GitOptions {
            should_commit: !input.no_commit && git_config.commit(),
            should_create_tags: !input.no_tags && git_config.tags(),
            should_delete_changesets: !input.keep_changesets && !git_config.keep_changesets(),
        };
        let is_prerelease_release = is_any_prerelease_configured(input, &per_package_config);

        self.validate_working_tree(&project.root, git_options.should_commit, input.dry_run)?;
        let inherited_packages =
            self.check_inherited_versions(&project.packages, input.convert_inherited)?;

        Ok(ReleaseContext {
            project,
            root_config,
            changeset_dir,
            changeset_files,
            prerelease_state,
            graduation_state,
            per_package_config,
            is_prerelease_graduation,
            is_graduating,
            is_prerelease_release,
            git_options,
            inherited_packages,
            early_return,
        })
    }

    fn check_early_return(
        changeset_files: &[PathBuf],
        is_graduating: bool,
        input: &ReleaseInput,
        per_package_config: &HashMap<String, PackageReleaseConfig>,
    ) -> Option<Result<ReleaseOutcome>> {
        if changeset_files.is_empty() && !is_graduating {
            if is_any_prerelease_configured(input, per_package_config) && !input.force {
                return Some(Err(OperationError::NoChangesetsWithoutForce));
            }
            return Some(Ok(ReleaseOutcome::NoChangesets));
        }
        None
    }

    fn plan_release(&self, context: &ReleaseContext, dry_run: bool) -> Result<ReleasePlan> {
        let (changesets, aggregator) =
            self.load_changesets(&context.changeset_dir, &context.changeset_files)?;

        let planned_releases = if context.is_prerelease_graduation {
            VersionPlanner::plan_graduation(&context.project.packages)?.releases
        } else {
            VersionPlanner::plan_releases_per_package(
                &changesets,
                &context.project.packages,
                &context.per_package_config,
                context.root_config.zero_version_behavior(),
            )?
            .releases
        };

        let package_lookup: IndexMap<_, _> = context
            .project
            .packages
            .iter()
            .map(|p| (p.name.clone(), p.clone()))
            .collect();

        let unchanged_packages =
            Self::collect_unchanged_packages(&context.project.packages, &planned_releases);

        let changelog_updates = if dry_run {
            Vec::new()
        } else {
            self.generate_changelog_updates(
                &context.project.root,
                context.root_config.changelog_config(),
                &aggregator,
                &planned_releases,
                &package_lookup,
            )?
        };

        let output = ReleaseOutput {
            planned_releases: planned_releases.clone(),
            unchanged_packages,
            changesets_consumed: context.changeset_files.clone(),
            changelog_updates,
            git_result: None,
        };

        Ok(ReleasePlan {
            output,
            planned_releases,
            package_lookup,
        })
    }

    fn execute_release(
        &self,
        context: &ReleaseContext,
        plan: ReleasePlan,
    ) -> Result<ReleaseOutcome> {
        self.write_manifest_versions(
            &context.project.root,
            &plan.package_lookup,
            &plan.planned_releases,
            &context.inherited_packages,
        )?;

        if let Some(first_release) = plan.planned_releases.first() {
            self.handle_changeset_consumption(
                &context.changeset_dir,
                &context.changeset_files,
                &first_release.new_version,
                context.is_prerelease_release,
                context.is_graduating,
            )?;
        }

        let should_delete_changesets_actual = context.git_options.should_delete_changesets
            && !context.is_prerelease_release
            && !context.is_prerelease_graduation;

        let git_result = self.perform_git_operations(
            &context.project.root,
            &context.project.kind,
            &plan.package_lookup,
            &plan.planned_releases,
            &plan.output.changelog_updates,
            &context.changeset_files,
            context.root_config.git_config(),
            context.git_options.should_commit,
            context.git_options.should_create_tags,
            should_delete_changesets_actual,
            &context.inherited_packages,
        )?;

        self.update_release_state(
            &context.changeset_dir,
            context.prerelease_state.as_ref(),
            context.graduation_state.as_ref(),
            &plan.planned_releases,
        )?;

        Ok(ReleaseOutcome::Executed(ReleaseOutput {
            git_result: Some(git_result),
            ..plan.output
        }))
    }

    fn build_cli_input(input: &ReleaseInput) -> ReleaseCliInput {
        ReleaseCliInput {
            cli_prerelease: input
                .per_package_config
                .iter()
                .filter_map(|(name, config)| {
                    config
                        .prerelease
                        .as_ref()
                        .map(|spec| (name.clone(), spec.clone()))
                })
                .collect(),
            global_prerelease: input.global_prerelease.clone(),
            cli_graduate: input
                .per_package_config
                .iter()
                .filter(|(_, config)| config.graduate_zero)
                .map(|(name, _)| name.clone())
                .collect(),
            graduate_all: input.graduate_all,
        }
    }

    fn update_release_state(
        &self,
        changeset_dir: &Path,
        prerelease_state: Option<&changeset_project::PrereleaseState>,
        graduation_state: Option<&GraduationState>,
        planned_releases: &[PackageVersion],
    ) -> Result<()> {
        if let Some(state) = prerelease_state {
            let mut new_state = state.clone();
            for release in planned_releases {
                let was_prerelease = changeset_version::is_prerelease(&release.current_version);
                let is_now_stable = !changeset_version::is_prerelease(&release.new_version);
                if was_prerelease && is_now_stable {
                    let _ = new_state.remove(&release.name);
                }
            }
            self.release_state_io
                .save_prerelease_state(changeset_dir, &new_state)?;
        }

        if let Some(state) = graduation_state {
            let mut new_state = state.clone();
            for release in planned_releases {
                if release.current_version.major == 0 && release.new_version.major >= 1 {
                    let _ = new_state.remove(&release.name);
                }
            }
            self.release_state_io
                .save_graduation_state(changeset_dir, &new_state)?;
        }

        Ok(())
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
        MockProjectProvider, MockReleaseStateIO, make_changeset,
    };

    fn default_input() -> ReleaseInput {
        ReleaseInput {
            dry_run: true,
            convert_inherited: false,
            no_commit: true,
            no_tags: true,
            keep_changesets: true,
            force: false,
            per_package_config: HashMap::new(),
            global_prerelease: None,
            graduate_all: false,
        }
    }

    fn make_operation<P, RW, M>(
        project_provider: P,
        changeset_io: RW,
        manifest_writer: M,
    ) -> ReleaseOperation<P, RW, M, MockChangelogWriter, MockGitProvider, MockReleaseStateIO>
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
            MockReleaseStateIO::new(),
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
            .with_changeset(PathBuf::from(".changeset/changesets/fix.md"), changeset);
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
            (PathBuf::from(".changeset/changesets/fix.md"), changeset1),
            (
                PathBuf::from(".changeset/changesets/feature.md"),
                changeset2,
            ),
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
            (
                PathBuf::from(".changeset/changesets/feature-a.md"),
                changeset1,
            ),
            (
                PathBuf::from(".changeset/changesets/breaking-b.md"),
                changeset2,
            ),
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
            .with_changeset(PathBuf::from(".changeset/changesets/fix.md"), changeset);
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
            (PathBuf::from(".changeset/changesets/fix1.md"), changeset1),
            (PathBuf::from(".changeset/changesets/fix2.md"), changeset2),
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
            .with_changeset(PathBuf::from(".changeset/changesets/fix.md"), changeset);
        let manifest_writer = MockManifestWriter::new();

        let operation = make_operation(project_provider, changeset_reader, manifest_writer);
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: true,
            no_tags: true,
            keep_changesets: true,
            force: false,
            per_package_config: HashMap::new(),
            global_prerelease: None,
            graduate_all: false,
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
            .with_changeset(PathBuf::from(".changeset/changesets/feature.md"), changeset);
        let manifest_writer = Arc::new(MockManifestWriter::new());

        let operation = ReleaseOperation::new(
            project_provider,
            changeset_reader,
            Arc::clone(&manifest_writer),
            MockChangelogWriter::new(),
            MockGitProvider::new(),
            MockReleaseStateIO::new(),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: true,
            no_tags: true,
            keep_changesets: true,
            force: false,
            per_package_config: HashMap::new(),
            global_prerelease: None,
            graduate_all: false,
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
            .with_changeset(PathBuf::from(".changeset/changesets/fix.md"), changeset);
        let manifest_writer = MockManifestWriter::new()
            .with_inherited(vec![PathBuf::from("/mock/project/Cargo.toml")]);

        let operation = make_operation(project_provider, changeset_reader, manifest_writer);
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: true,
            no_tags: true,
            keep_changesets: true,
            force: false,
            per_package_config: HashMap::new(),
            global_prerelease: None,
            graduate_all: false,
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
            .with_changeset(PathBuf::from(".changeset/changesets/fix.md"), changeset);
        let manifest_writer = MockManifestWriter::new()
            .with_inherited(vec![PathBuf::from("/mock/project/Cargo.toml")]);

        let operation = make_operation(project_provider, changeset_reader, manifest_writer);
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: true,
            no_commit: true,
            no_tags: true,
            keep_changesets: true,
            force: false,
            per_package_config: HashMap::new(),
            global_prerelease: None,
            graduate_all: false,
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
            .with_changeset(PathBuf::from(".changeset/changesets/fix.md"), changeset);
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
            MockReleaseStateIO::new(),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: true,
            no_commit: true,
            no_tags: true,
            keep_changesets: true,
            force: false,
            per_package_config: HashMap::new(),
            global_prerelease: None,
            graduate_all: false,
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
            .with_changeset(PathBuf::from(".changeset/changesets/fix.md"), changeset);
        let manifest_writer = MockManifestWriter::new();
        let git_provider = MockGitProvider::new().is_clean(false);

        let operation = ReleaseOperation::new(
            project_provider,
            changeset_reader,
            manifest_writer,
            MockChangelogWriter::new(),
            git_provider,
            MockReleaseStateIO::new(),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: false,
            no_tags: true,
            keep_changesets: true,
            force: false,
            per_package_config: HashMap::new(),
            global_prerelease: None,
            graduate_all: false,
        };

        let result = operation.execute(Path::new("/any"), &input);

        assert!(matches!(result, Err(OperationError::DirtyWorkingTree)));
    }

    #[test]
    fn allows_dirty_tree_with_no_commit() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset = make_changeset("my-crate", BumpType::Patch, "Fix");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/changesets/fix.md"), changeset);
        let manifest_writer = MockManifestWriter::new();
        let git_provider = MockGitProvider::new().is_clean(false);

        let operation = ReleaseOperation::new(
            project_provider,
            changeset_reader,
            manifest_writer,
            MockChangelogWriter::new(),
            git_provider,
            MockReleaseStateIO::new(),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: true,
            no_tags: true,
            keep_changesets: true,
            force: false,
            per_package_config: HashMap::new(),
            global_prerelease: None,
            graduate_all: false,
        };

        let result = operation.execute(Path::new("/any"), &input);

        assert!(result.is_ok());
    }

    #[test]
    fn allows_dirty_tree_in_dry_run() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset = make_changeset("my-crate", BumpType::Patch, "Fix");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/changesets/fix.md"), changeset);
        let manifest_writer = MockManifestWriter::new();
        let git_provider = MockGitProvider::new().is_clean(false);

        let operation = ReleaseOperation::new(
            project_provider,
            changeset_reader,
            manifest_writer,
            MockChangelogWriter::new(),
            git_provider,
            MockReleaseStateIO::new(),
        );
        let input = ReleaseInput {
            dry_run: true,
            convert_inherited: false,
            no_commit: false,
            no_tags: false,
            keep_changesets: false,
            force: false,
            per_package_config: HashMap::new(),
            global_prerelease: None,
            graduate_all: false,
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
            .with_changeset(PathBuf::from(".changeset/changesets/feature.md"), changeset);
        let manifest_writer = MockManifestWriter::new();
        let git_provider = Arc::new(MockGitProvider::new());

        let operation = ReleaseOperation::new(
            project_provider,
            changeset_reader,
            manifest_writer,
            MockChangelogWriter::new(),
            Arc::clone(&git_provider),
            MockReleaseStateIO::new(),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: false,
            no_tags: true,
            keep_changesets: true,
            force: false,
            per_package_config: HashMap::new(),
            global_prerelease: None,
            graduate_all: false,
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
            (PathBuf::from(".changeset/changesets/fix-a.md"), changeset1),
            (PathBuf::from(".changeset/changesets/fix-b.md"), changeset2),
        ]);
        let manifest_writer = MockManifestWriter::new();
        let git_provider = Arc::new(MockGitProvider::new());

        let operation = ReleaseOperation::new(
            project_provider,
            changeset_reader,
            manifest_writer,
            MockChangelogWriter::new(),
            Arc::clone(&git_provider),
            MockReleaseStateIO::new(),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: false,
            no_tags: false,
            keep_changesets: true,
            force: false,
            per_package_config: HashMap::new(),
            global_prerelease: None,
            graduate_all: false,
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
            .with_changeset(PathBuf::from(".changeset/changesets/fix.md"), changeset);
        let manifest_writer = MockManifestWriter::new();
        let git_provider = Arc::new(MockGitProvider::new());

        let operation = ReleaseOperation::new(
            project_provider,
            changeset_reader,
            manifest_writer,
            MockChangelogWriter::new(),
            Arc::clone(&git_provider),
            MockReleaseStateIO::new(),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: false,
            no_tags: true,
            keep_changesets: true,
            force: false,
            per_package_config: HashMap::new(),
            global_prerelease: None,
            graduate_all: false,
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
            .with_changeset(PathBuf::from(".changeset/changesets/fix.md"), changeset);
        let manifest_writer = MockManifestWriter::new();
        let git_provider = Arc::new(MockGitProvider::new());

        let operation = ReleaseOperation::new(
            project_provider,
            changeset_reader,
            manifest_writer,
            MockChangelogWriter::new(),
            Arc::clone(&git_provider),
            MockReleaseStateIO::new(),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: false,
            no_tags: false,
            keep_changesets: true,
            force: false,
            per_package_config: HashMap::new(),
            global_prerelease: None,
            graduate_all: false,
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
            .with_changeset(PathBuf::from(".changeset/changesets/fix.md"), changeset);
        let manifest_writer = MockManifestWriter::new();
        let git_provider = Arc::new(MockGitProvider::new());

        let operation = ReleaseOperation::new(
            project_provider,
            changeset_reader,
            manifest_writer,
            MockChangelogWriter::new(),
            Arc::clone(&git_provider),
            MockReleaseStateIO::new(),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: true,
            no_tags: true,
            keep_changesets: false,
            force: false,
            per_package_config: HashMap::new(),
            global_prerelease: None,
            graduate_all: false,
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
            PathBuf::from(".changeset/changesets/fix.md")
        );
    }

    #[test]
    fn keep_changesets_true_leaves_deleted_list_empty() {
        use std::sync::Arc;

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset = make_changeset("my-crate", BumpType::Patch, "Fix");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/changesets/fix.md"), changeset);
        let manifest_writer = MockManifestWriter::new();
        let git_provider = Arc::new(MockGitProvider::new());

        let operation = ReleaseOperation::new(
            project_provider,
            changeset_reader,
            manifest_writer,
            MockChangelogWriter::new(),
            Arc::clone(&git_provider),
            MockReleaseStateIO::new(),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: true,
            no_tags: true,
            keep_changesets: true,
            force: false,
            per_package_config: HashMap::new(),
            global_prerelease: None,
            graduate_all: false,
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
            (PathBuf::from(".changeset/changesets/fix1.md"), changeset1),
            (PathBuf::from(".changeset/changesets/fix2.md"), changeset2),
        ]);
        let manifest_writer = MockManifestWriter::new();
        let git_provider = Arc::new(MockGitProvider::new());

        let operation = ReleaseOperation::new(
            project_provider,
            changeset_reader,
            manifest_writer,
            MockChangelogWriter::new(),
            Arc::clone(&git_provider),
            MockReleaseStateIO::new(),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: false,
            no_tags: true,
            keep_changesets: false,
            force: false,
            per_package_config: HashMap::new(),
            global_prerelease: None,
            graduate_all: false,
        };

        let _ = operation
            .execute(Path::new("/any"), &input)
            .expect("execute failed");

        let staged = git_provider.staged_files();
        assert!(
            staged.contains(&PathBuf::from(".changeset/changesets/fix1.md")),
            "fix1.md should be staged"
        );
        assert!(
            staged.contains(&PathBuf::from(".changeset/changesets/fix2.md")),
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
            .with_changeset(PathBuf::from(".changeset/changesets/feature.md"), changeset);
        let manifest_writer = MockManifestWriter::new();
        let git_provider = Arc::new(MockGitProvider::new());

        let operation = ReleaseOperation::new(
            project_provider,
            changeset_reader,
            manifest_writer,
            MockChangelogWriter::new(),
            Arc::clone(&git_provider),
            MockReleaseStateIO::new(),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: false,
            no_tags: true,
            keep_changesets: true,
            force: false,
            per_package_config: HashMap::new(),
            global_prerelease: None,
            graduate_all: false,
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
    fn prerelease_marks_changesets_as_consumed() {
        use std::sync::Arc;

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset_path = PathBuf::from(".changeset/changesets/fix.md");
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
            MockReleaseStateIO::new(),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: true,
            no_tags: true,
            keep_changesets: true,
            force: false,
            per_package_config: HashMap::new(),
            global_prerelease: Some(PrereleaseSpec::Alpha),
            graduate_all: false,
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
    fn prerelease_increment_requires_changesets_or_force() {
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
            force: false,
            per_package_config: HashMap::new(),
            global_prerelease: Some(PrereleaseSpec::Alpha),
            graduate_all: false,
        };

        let result = operation.execute(Path::new("/any"), &input);

        assert!(
            matches!(result, Err(OperationError::NoChangesetsWithoutForce)),
            "should error without changesets and without force flag"
        );
    }

    #[test]
    fn prerelease_with_force_returns_no_changesets() {
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
            force: true,
            per_package_config: HashMap::new(),
            global_prerelease: Some(PrereleaseSpec::Alpha),
            graduate_all: false,
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
    fn graduation_clears_consumed_flag() {
        use std::sync::Arc;

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.1-alpha.1");
        let consumed_path = PathBuf::from(".changeset/changesets/consumed.md");
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
            MockReleaseStateIO::new(),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: true,
            no_tags: true,
            keep_changesets: true,
            force: false,
            per_package_config: HashMap::new(),
            global_prerelease: None,
            graduate_all: false,
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
    fn graduation_aggregates_consumed_changesets_in_changelog() {
        use std::sync::Arc;

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.1-alpha.1");
        let consumed_path1 = PathBuf::from(".changeset/changesets/fix1.md");
        let consumed_path2 = PathBuf::from(".changeset/changesets/fix2.md");
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
            MockReleaseStateIO::new(),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: true,
            no_tags: true,
            keep_changesets: true,
            force: false,
            per_package_config: HashMap::new(),
            global_prerelease: None,
            graduate_all: false,
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
    fn consumed_changesets_excluded_from_normal_release() {
        use std::sync::Arc;

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let unconsumed_path = PathBuf::from(".changeset/changesets/unconsumed.md");
        let consumed_path = PathBuf::from(".changeset/changesets/consumed.md");
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
            MockReleaseStateIO::new(),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: true,
            no_tags: true,
            keep_changesets: true,
            force: false,
            per_package_config: HashMap::new(),
            global_prerelease: None,
            graduate_all: false,
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
    fn prerelease_with_different_tag_resets_number() {
        use std::sync::Arc;

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.1-alpha.2");
        let changeset_path = PathBuf::from(".changeset/changesets/feature.md");
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
            MockReleaseStateIO::new(),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: true,
            no_tags: true,
            keep_changesets: true,
            force: false,
            per_package_config: HashMap::new(),
            global_prerelease: Some(PrereleaseSpec::Beta),
            graduate_all: false,
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

    #[test]
    fn zero_graduation_deletes_changesets() {
        use std::sync::Arc;

        let project_provider = MockProjectProvider::single_package("my-crate", "0.5.0");
        let changeset_path = PathBuf::from(".changeset/changesets/feature.md");
        let changeset = make_changeset("my-crate", BumpType::Minor, "Add feature");
        let changeset_reader =
            Arc::new(MockChangesetReader::new().with_changeset(changeset_path.clone(), changeset));
        let manifest_writer = MockManifestWriter::new();
        let git_provider = Arc::new(MockGitProvider::new());

        let operation = ReleaseOperation::new(
            project_provider,
            Arc::clone(&changeset_reader),
            manifest_writer,
            MockChangelogWriter::new(),
            Arc::clone(&git_provider),
            MockReleaseStateIO::new(),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: true,
            no_tags: true,
            keep_changesets: false,
            force: false,
            per_package_config: HashMap::new(),
            global_prerelease: None,
            graduate_all: true,
        };

        let ReleaseOutcome::Executed(output) = operation
            .execute(Path::new("/any"), &input)
            .expect("zero graduation should succeed")
        else {
            panic!("expected Executed outcome");
        };

        assert_eq!(
            output.planned_releases[0].new_version.to_string(),
            "1.0.0",
            "zero graduation should bump to 1.0.0"
        );

        let git_result = output.git_result.expect("should have git result");
        assert_eq!(
            git_result.changesets_deleted.len(),
            1,
            "zero graduation should delete changesets"
        );
        assert!(
            git_result.changesets_deleted.contains(&changeset_path),
            "deleted list should contain the changeset file"
        );

        let deleted_files = git_provider.deleted_files();
        assert!(
            deleted_files.contains(&changeset_path),
            "changeset file should be deleted via git provider"
        );
    }

    #[test]
    fn prerelease_graduation_preserves_changesets() {
        use std::sync::Arc;

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.1-alpha.1");
        let consumed_path = PathBuf::from(".changeset/changesets/consumed.md");
        let changeset = make_changeset("my-crate", BumpType::Patch, "Fix bug");
        let changeset_reader = Arc::new(MockChangesetReader::new().with_consumed_changeset(
            consumed_path.clone(),
            changeset,
            "1.0.1-alpha.1".to_string(),
        ));
        let manifest_writer = MockManifestWriter::new();
        let git_provider = Arc::new(MockGitProvider::new());

        let operation = ReleaseOperation::new(
            project_provider,
            Arc::clone(&changeset_reader),
            manifest_writer,
            MockChangelogWriter::new(),
            Arc::clone(&git_provider),
            MockReleaseStateIO::new(),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: true,
            no_tags: true,
            keep_changesets: false,
            force: false,
            per_package_config: HashMap::new(),
            global_prerelease: None,
            graduate_all: false,
        };

        let ReleaseOutcome::Executed(output) = operation
            .execute(Path::new("/any"), &input)
            .expect("prerelease graduation should succeed")
        else {
            panic!("expected Executed outcome");
        };

        assert_eq!(
            output.planned_releases[0].new_version.to_string(),
            "1.0.1",
            "prerelease graduation should remove prerelease suffix"
        );

        let git_result = output.git_result.expect("should have git result");
        assert!(
            git_result.changesets_deleted.is_empty(),
            "prerelease graduation should NOT delete changesets (they were already consumed)"
        );

        let deleted_files = git_provider.deleted_files();
        assert!(
            deleted_files.is_empty(),
            "no files should be deleted during prerelease graduation"
        );
    }

    #[test]
    fn release_respects_prerelease_toml_state() {
        use changeset_project::PrereleaseState;
        use std::sync::Arc;

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset = make_changeset("my-crate", BumpType::Patch, "Fix bug");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/changesets/fix.md"), changeset);
        let manifest_writer = MockManifestWriter::new();

        let mut prerelease_state = PrereleaseState::new();
        prerelease_state.insert("my-crate".to_string(), "alpha".to_string());
        let release_state_io =
            Arc::new(MockReleaseStateIO::new().with_prerelease_state(prerelease_state));

        let operation = ReleaseOperation::new(
            project_provider,
            changeset_reader,
            manifest_writer,
            MockChangelogWriter::new(),
            MockGitProvider::new(),
            Arc::clone(&release_state_io),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: true,
            no_tags: true,
            keep_changesets: true,
            force: false,
            per_package_config: HashMap::new(),
            global_prerelease: None,
            graduate_all: false,
        };

        let ReleaseOutcome::Executed(output) = operation
            .execute(Path::new("/any"), &input)
            .expect("release should succeed")
        else {
            panic!("expected Executed outcome");
        };

        assert_eq!(output.planned_releases.len(), 1);
        assert_eq!(
            output.planned_releases[0].new_version.to_string(),
            "1.0.1-alpha.1",
            "should apply prerelease from TOML state"
        );
    }

    #[test]
    fn cli_prerelease_overrides_toml_state() {
        use changeset_project::PrereleaseState;
        use std::sync::Arc;

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset = make_changeset("my-crate", BumpType::Patch, "Fix bug");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/changesets/fix.md"), changeset);
        let manifest_writer = MockManifestWriter::new();

        let mut prerelease_state = PrereleaseState::new();
        prerelease_state.insert("my-crate".to_string(), "alpha".to_string());
        let release_state_io =
            Arc::new(MockReleaseStateIO::new().with_prerelease_state(prerelease_state));

        let operation = ReleaseOperation::new(
            project_provider,
            changeset_reader,
            manifest_writer,
            MockChangelogWriter::new(),
            MockGitProvider::new(),
            Arc::clone(&release_state_io),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: true,
            no_tags: true,
            keep_changesets: true,
            force: false,
            per_package_config: HashMap::new(),
            global_prerelease: Some(PrereleaseSpec::Beta),
            graduate_all: false,
        };

        let ReleaseOutcome::Executed(output) = operation
            .execute(Path::new("/any"), &input)
            .expect("release should succeed")
        else {
            panic!("expected Executed outcome");
        };

        assert_eq!(output.planned_releases.len(), 1);
        assert_eq!(
            output.planned_releases[0].new_version.to_string(),
            "1.0.1-beta.1",
            "CLI prerelease should override TOML state"
        );
    }

    #[test]
    fn graduation_state_updates_after_release() {
        use changeset_project::GraduationState;
        use std::sync::Arc;

        let project_provider = MockProjectProvider::single_package("my-crate", "0.5.0");
        let changeset = make_changeset("my-crate", BumpType::Minor, "Add feature");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/changesets/feature.md"), changeset);
        let manifest_writer = MockManifestWriter::new();

        let mut graduation_state = GraduationState::new();
        graduation_state.add("my-crate".to_string());
        let release_state_io =
            Arc::new(MockReleaseStateIO::new().with_graduation_state(graduation_state));

        let operation = ReleaseOperation::new(
            project_provider,
            changeset_reader,
            manifest_writer,
            MockChangelogWriter::new(),
            MockGitProvider::new(),
            Arc::clone(&release_state_io),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: true,
            no_tags: true,
            keep_changesets: true,
            force: false,
            per_package_config: HashMap::new(),
            global_prerelease: None,
            graduate_all: false,
        };

        let ReleaseOutcome::Executed(output) = operation
            .execute(Path::new("/any"), &input)
            .expect("release should succeed")
        else {
            panic!("expected Executed outcome");
        };

        assert_eq!(output.planned_releases.len(), 1);
        assert_eq!(
            output.planned_releases[0].new_version.to_string(),
            "1.0.0",
            "should graduate from 0.x to 1.0.0"
        );

        let updated_state = release_state_io.get_graduation_state();
        assert!(
            updated_state.is_none() || !updated_state.expect("state").contains("my-crate"),
            "graduated package should be removed from graduation state"
        );
    }

    #[test]
    fn graduate_all_flag_graduates_zero_versions() {
        let project_provider = MockProjectProvider::single_package("my-crate", "0.5.0");
        let changeset = make_changeset("my-crate", BumpType::Patch, "Fix bug");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/changesets/fix.md"), changeset);
        let manifest_writer = MockManifestWriter::new();

        let operation = make_operation(project_provider, changeset_reader, manifest_writer);
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: true,
            no_tags: true,
            keep_changesets: true,
            force: false,
            per_package_config: HashMap::new(),
            global_prerelease: None,
            graduate_all: true,
        };

        let ReleaseOutcome::Executed(output) = operation
            .execute(Path::new("/any"), &input)
            .expect("release should succeed")
        else {
            panic!("expected Executed outcome");
        };

        assert_eq!(output.planned_releases.len(), 1);
        assert_eq!(
            output.planned_releases[0].new_version.to_string(),
            "1.0.0",
            "graduate_all should promote 0.x to 1.0.0"
        );
    }

    #[test]
    fn prerelease_state_saved_after_normal_release() {
        use changeset_project::PrereleaseState;
        use std::sync::Arc;

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset_path = PathBuf::from(".changeset/changesets/fix.md");
        let changeset = make_changeset("my-crate", BumpType::Patch, "Fix bug");
        let changeset_reader =
            Arc::new(MockChangesetReader::new().with_changeset(changeset_path, changeset));
        let manifest_writer = MockManifestWriter::new();

        let mut prerelease_state = PrereleaseState::new();
        prerelease_state.insert("other-crate".to_string(), "beta".to_string());
        let release_state_io =
            Arc::new(MockReleaseStateIO::new().with_prerelease_state(prerelease_state));

        let operation = ReleaseOperation::new(
            project_provider,
            Arc::clone(&changeset_reader),
            manifest_writer,
            MockChangelogWriter::new(),
            MockGitProvider::new(),
            Arc::clone(&release_state_io),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: true,
            no_tags: true,
            keep_changesets: true,
            force: false,
            per_package_config: HashMap::new(),
            global_prerelease: None,
            graduate_all: false,
        };

        let ReleaseOutcome::Executed(output) = operation
            .execute(Path::new("/any"), &input)
            .expect("release should succeed")
        else {
            panic!("expected Executed outcome");
        };

        assert_eq!(output.planned_releases.len(), 1);
        assert_eq!(
            output.planned_releases[0].new_version.to_string(),
            "1.0.1",
            "should bump patch version"
        );

        let updated_state = release_state_io.get_prerelease_state();
        assert!(
            updated_state
                .as_ref()
                .is_some_and(|s| s.contains("other-crate")),
            "unrelated packages should remain in prerelease state after release"
        );
    }

    #[test]
    fn prerelease_graduation_removes_package_from_state_if_present() {
        use std::sync::Arc;

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0-alpha.1");
        let consumed_path = PathBuf::from(".changeset/changesets/fix.md");
        let changeset = make_changeset("my-crate", BumpType::Patch, "Fix bug");
        let changeset_reader = Arc::new(MockChangesetReader::new().with_consumed_changeset(
            consumed_path,
            changeset,
            "1.0.0-alpha.1".to_string(),
        ));
        let manifest_writer = MockManifestWriter::new();

        let release_state_io = Arc::new(MockReleaseStateIO::new());

        let operation = ReleaseOperation::new(
            project_provider,
            Arc::clone(&changeset_reader),
            manifest_writer,
            MockChangelogWriter::new(),
            MockGitProvider::new(),
            Arc::clone(&release_state_io),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
            no_commit: true,
            no_tags: true,
            keep_changesets: true,
            force: false,
            per_package_config: HashMap::new(),
            global_prerelease: None,
            graduate_all: false,
        };

        let ReleaseOutcome::Executed(output) = operation
            .execute(Path::new("/any"), &input)
            .expect("graduation should succeed")
        else {
            panic!("expected Executed outcome");
        };

        assert_eq!(output.planned_releases.len(), 1);
        assert_eq!(
            output.planned_releases[0].new_version.to_string(),
            "1.0.0",
            "should graduate from prerelease to stable"
        );
        assert!(
            changeset_version::is_prerelease(&output.planned_releases[0].current_version),
            "current version should have been a prerelease"
        );
        assert!(
            !changeset_version::is_prerelease(&output.planned_releases[0].new_version),
            "new version should be stable"
        );

        let updated_state = release_state_io.get_prerelease_state();
        assert!(
            updated_state.is_none() || !updated_state.expect("state").contains("my-crate"),
            "graduated package should not be in prerelease state"
        );
    }
}
