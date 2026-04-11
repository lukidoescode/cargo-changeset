use std::path::{Path, PathBuf};
use std::sync::Arc;

use changeset_core::{CARGO_MANIFEST_FILENAME, PackageInfo};
use changeset_project::{ProjectKind, TagFormat, WorkspaceDependencyGraph};
use changeset_saga::SagaBuilder;
use chrono::Local;
use indexmap::IndexMap;

use super::classifiers::{self, EarlyReturnDecision};
use super::context::ReleaseSagaContext;
use super::saga_data::{ReleaseSagaData, SagaReleaseOptions};
use super::saga_steps::{
    ClearChangesetsConsumedStep, CreateCommitStep, CreateTagsStep, DeleteChangesetFilesStep,
    MarkChangesetsConsumedStep, RemoveWorkspaceVersionStep, RestoreChangelogsStep, StageFilesStep,
    UpdateDependencyVersionsStep, UpdateLockfileStep, UpdateReleaseStateStep,
    WriteManifestVersionsStep,
};
use super::types::{
    ChangelogUpdate, GitOptions, PrepareResult, ReleaseClassification, ReleaseContext,
    ReleaseInput, ReleaseOutcome, ReleaseOutput, ReleasePlan,
};
use super::validator::{ReleaseCliInput, ReleaseValidator};
use crate::Result;
use crate::error::OperationError;
use crate::none_bump;
use crate::operations::changelog_aggregation::ChangesetAggregator;
use crate::planner::VersionPlanner;
use crate::traits::{
    ChangelogWriter, ChangesetReader, ChangesetWriter, DependencyGraphProvider, FullGitProvider,
    FullManifestWriter, ProjectProvider, ReleaseStateIO,
};
use crate::types::PackageVersion;

pub struct ReleaseOperation<P, RW, M, C, G, S> {
    project_provider: P,
    changeset_io: Arc<RW>,
    manifest_writer: Arc<M>,
    changelog_writer: Arc<C>,
    git_provider: Arc<G>,
    release_state_io: Arc<S>,
}

impl<P, RW, M, C, G, S> ReleaseOperation<P, RW, M, C, G, S>
where
    P: ProjectProvider + DependencyGraphProvider,
    RW: ChangesetReader + ChangesetWriter + Send + Sync + 'static,
    M: FullManifestWriter + Send + Sync + 'static,
    C: ChangelogWriter + Send + Sync + 'static,
    G: FullGitProvider + Send + Sync + 'static,
    S: ReleaseStateIO + Send + Sync + 'static,
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
            changeset_io: Arc::new(changeset_io),
            manifest_writer: Arc::new(manifest_writer),
            changelog_writer: Arc::new(changelog_writer),
            git_provider: Arc::new(git_provider),
            release_state_io: Arc::new(release_state_io),
        }
    }

    fn capture_changelog_state(
        &self,
        project_root: &Path,
        strategy: &dyn super::changelog_strategy::ChangelogHandler,
        planned_releases: &[PackageVersion],
        package_lookup: &IndexMap<String, PackageInfo>,
    ) -> Result<Vec<super::types::ChangelogFileState>> {
        let ctx = super::changelog_strategy::ChangelogCaptureContext {
            project_root,
            planned_releases,
            package_lookup,
            changelog_writer: self.changelog_writer.as_ref(),
        };
        strategy.capture_state(&ctx)
    }

    fn generate_changelog_updates(
        &self,
        project_root: &Path,
        changelog_config: &changeset_changelog::ChangelogConfig,
        strategy: &dyn super::changelog_strategy::ChangelogHandler,
        aggregator: &ChangesetAggregator,
        planned_releases: &[PackageVersion],
        package_lookup: &IndexMap<String, PackageInfo>,
    ) -> Result<Vec<ChangelogUpdate>> {
        let today = Local::now().date_naive();
        let repo_info = super::loading::resolve_repo_info(
            self.git_provider.as_ref(),
            project_root,
            changelog_config,
        )?;
        let ctx = super::changelog_strategy::ChangelogGenerateContext {
            project_root,
            aggregator,
            planned_releases,
            package_lookup,
            repo_info: repo_info.as_ref(),
            today,
            changelog_writer: self.changelog_writer.as_ref(),
        };
        strategy.generate_updates(&ctx)
    }

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

    fn check_inherited_versions(
        &self,
        packages: &[PackageInfo],
        convert_inherited: bool,
    ) -> Result<Vec<String>> {
        let inherited_packages = self
            .manifest_writer
            .find_packages_with_inherited_versions(packages)?;
        if !inherited_packages.is_empty() && !convert_inherited {
            return Err(OperationError::InheritedVersionsRequireConvert {
                packages: inherited_packages,
            });
        }
        Ok(inherited_packages)
    }

    /// # Errors
    ///
    /// Propagates errors from project discovery, changeset parsing, version
    /// planning, changelog generation, and git operations.
    pub fn execute(&self, start_path: &Path, input: &ReleaseInput) -> Result<ReleaseOutcome> {
        let context = match self.prepare_release_context(start_path, input)? {
            PrepareResult::Ready(ctx) => *ctx,
            PrepareResult::EarlyReturn(outcome) => return Ok(outcome),
        };

        let plan = self.plan_release(&context, input.dry_run())?;

        if input.dry_run() {
            return Ok(ReleaseOutcome::DryRun(plan.output));
        }

        self.execute_release(&context, plan)
    }

    fn prepare_release_context(
        &self,
        start_path: &Path,
        input: &ReleaseInput,
    ) -> Result<PrepareResult> {
        let project = self.project_provider.discover_project(start_path)?;
        let (root_config, _) = self.project_provider.load_configs(&project)?;

        let changeset_dir = project.root().join(root_config.changeset_dir());
        let changeset_files = self.changeset_io.list_changesets(&changeset_dir)?;

        let prerelease_state = self
            .release_state_io
            .load_prerelease_state(&changeset_dir)?;
        let graduation_state = self
            .release_state_io
            .load_graduation_state(&changeset_dir)?;

        let cli_input = ReleaseCliInput::from(input);
        let validated_config = ReleaseValidator::validate(
            &cli_input,
            prerelease_state.as_ref(),
            graduation_state.as_ref(),
            project.packages(),
            project.kind(),
        )?;

        let per_package_config = validated_config.into_per_package();

        let is_prerelease_graduation =
            classifiers::is_prerelease_graduation(project.packages(), &per_package_config);
        let is_zero_graduation =
            classifiers::is_zero_graduation(project.packages(), input, &per_package_config);
        let is_graduating = is_prerelease_graduation || is_zero_graduation;

        match classifiers::check_early_return(
            &changeset_files,
            is_graduating,
            input,
            &per_package_config,
        ) {
            EarlyReturnDecision::NoChangesets => {
                return Ok(PrepareResult::EarlyReturn(ReleaseOutcome::NoChangesets));
            }
            EarlyReturnDecision::ForceRequired => {
                return Err(OperationError::NoChangesetsWithoutForce);
            }
            EarlyReturnDecision::Continue => {}
        }

        let git_config = root_config.git_config();
        let git_options = GitOptions {
            should_commit: !input.no_commit() && git_config.commit(),
            should_create_tags: !input.no_tags() && git_config.tags(),
            should_delete_changesets: !input.keep_changesets() && !git_config.keep_changesets(),
        };
        let is_prerelease_release =
            classifiers::is_any_prerelease_configured(input, &per_package_config);

        self.validate_working_tree(project.root(), git_options.should_commit, input.dry_run())?;
        let inherited_packages =
            self.check_inherited_versions(project.packages(), input.convert_inherited())?;

        Ok(PrepareResult::Ready(Box::new(ReleaseContext {
            project,
            root_config,
            changeset_dir,
            changeset_files,
            prerelease_state,
            graduation_state,
            per_package_config,
            classification: ReleaseClassification {
                is_prerelease_graduation,
                is_graduating,
                is_prerelease_release,
            },
            git_options,
            inherited_packages,
        })))
    }

    fn plan_release(&self, context: &ReleaseContext, dry_run: bool) -> Result<ReleasePlan> {
        let (changesets, mut aggregator) = super::loading::load_changesets(
            self.changeset_io.as_ref(),
            &context.changeset_dir,
            &context.changeset_files,
        )?;

        let planned_releases = if context.classification.is_prerelease_graduation {
            VersionPlanner::plan_graduation(context.project.packages())?
                .releases()
                .clone()
        } else {
            let changesets = none_bump::apply_none_bump_behavior(
                changesets,
                context.root_config.none_bump_behavior(),
                context.root_config.none_bump_promote_message_template(),
            )?;

            aggregator = ChangesetAggregator::new();
            for cs in &changesets {
                aggregator.add_changeset(cs);
            }

            let consumed_paths = self
                .changeset_io
                .list_consumed_changesets(&context.changeset_dir)?;
            for path in &consumed_paths {
                let consumed_cs = self.changeset_io.read_changeset(path)?;
                aggregator.add_changeset(&consumed_cs);
            }

            let base_releases = VersionPlanner::plan_releases_per_package(
                &changesets,
                context.project.packages(),
                &context.per_package_config,
                context.root_config.zero_version_behavior(),
            )?
            .releases()
            .clone();

            let graph = self
                .project_provider
                .build_dependency_graph(&context.project)?;
            let expanded = super::dependency_expansion::expand_with_reverse_dependencies(
                base_releases,
                &graph,
                context.project.packages(),
                context.root_config.zero_version_behavior(),
            )?;

            populate_dependency_update_entries(
                &expanded,
                &graph,
                context.root_config.dependency_bump_changelog_template(),
                &mut aggregator,
            );

            expanded
        };

        let package_lookup: IndexMap<_, _> = context
            .project
            .packages()
            .iter()
            .map(|p| (p.name().clone(), p.clone()))
            .collect();

        let unchanged_packages =
            classifiers::collect_unchanged_packages(context.project.packages(), &planned_releases);

        let (changelog_updates, changelog_backups) = if dry_run {
            (Vec::new(), Vec::new())
        } else {
            let strategy = super::changelog_strategy::strategy_for(
                context.root_config.changelog_config().changelog(),
            );
            let backups = self.capture_changelog_state(
                context.project.root(),
                strategy.as_ref(),
                &planned_releases,
                &package_lookup,
            )?;
            let updates = self.generate_changelog_updates(
                context.project.root(),
                context.root_config.changelog_config(),
                strategy.as_ref(),
                &aggregator,
                &planned_releases,
                &package_lookup,
            )?;
            (updates, backups)
        };

        let output = ReleaseOutput::new(
            planned_releases,
            unchanged_packages,
            context.changeset_files.clone(),
            changelog_updates,
            None,
        );

        Ok(ReleasePlan {
            output,
            package_lookup,
            changelog_backups,
        })
    }

    fn execute_release(
        &self,
        context: &ReleaseContext,
        plan: ReleasePlan,
    ) -> Result<ReleaseOutcome> {
        let package_paths: IndexMap<String, PathBuf> = plan
            .package_lookup
            .iter()
            .map(|(name, info)| (name.clone(), info.path().clone()))
            .collect();

        let saga_data = ReleaseSagaData::new(
            context.changeset_dir.clone(),
            context.project.root().join(CARGO_MANIFEST_FILENAME),
            plan.output.planned_releases().clone(),
            package_paths,
            plan.output.changelog_updates().clone(),
            context.changeset_files.clone(),
        )
        .with_options(SagaReleaseOptions {
            classification: context.classification,
            git_options: context.git_options,
        })
        .with_inherited_packages(context.inherited_packages.clone())
        .with_prerelease_state(context.prerelease_state.as_ref())
        .with_graduation_state(context.graduation_state.as_ref())
        .with_changelog_backups(plan.changelog_backups);

        let result = self.execute_release_saga(context, saga_data)?;

        Ok(ReleaseOutcome::Executed(
            plan.output.with_git_result(result.into_git_result()),
        ))
    }

    fn execute_release_saga(
        &self,
        context: &ReleaseContext,
        saga_data: ReleaseSagaData,
    ) -> Result<ReleaseSagaData> {
        type RestoreChangelogs<G, M, RW, S, CW> = RestoreChangelogsStep<G, M, RW, S, CW>;
        type WriteManifests<G, M, RW, S, CW> = WriteManifestVersionsStep<G, M, RW, S, CW>;
        type UpdateDeps<G, M, RW, S, CW> = UpdateDependencyVersionsStep<G, M, RW, S, CW>;
        type RemoveWorkspace<G, M, RW, S, CW> = RemoveWorkspaceVersionStep<G, M, RW, S, CW>;
        type UpdateLockfile<G, M, RW, S, CW> = UpdateLockfileStep<G, M, RW, S, CW>;
        type MarkConsumed<G, M, RW, S, CW> = MarkChangesetsConsumedStep<G, M, RW, S, CW>;
        type ClearConsumed<G, M, RW, S, CW> = ClearChangesetsConsumedStep<G, M, RW, S, CW>;
        type DeleteChangesets<G, M, RW, S, CW> = DeleteChangesetFilesStep<G, M, RW, S, CW>;
        type Stage<G, M, RW, S, CW> = StageFilesStep<G, M, RW, S, CW>;
        type Commit<G, M, RW, S, CW> = CreateCommitStep<G, M, RW, S, CW>;
        type Tags<G, M, RW, S, CW> = CreateTagsStep<G, M, RW, S, CW>;
        type UpdateState<G, M, RW, S, CW> = UpdateReleaseStateStep<G, M, RW, S, CW>;

        let git_config = context.root_config.git_config();
        let use_crate_prefix = match context.project.kind() {
            ProjectKind::SinglePackage => git_config.tag_format() == TagFormat::CratePrefixed,
            ProjectKind::VirtualWorkspace | ProjectKind::WorkspaceWithRoot => true,
        };

        let saga = SagaBuilder::new()
            .first_step(RestoreChangelogs::<G, M, RW, S, C>::new())
            .then(WriteManifests::<G, M, RW, S, C>::new())
            .then(UpdateDeps::<G, M, RW, S, C>::new())
            .then(RemoveWorkspace::<G, M, RW, S, C>::new())
            .then(UpdateLockfile::<G, M, RW, S, C>::new())
            .then(MarkConsumed::<G, M, RW, S, C>::new())
            .then(ClearConsumed::<G, M, RW, S, C>::new())
            .then(DeleteChangesets::<G, M, RW, S, C>::new())
            .then(Stage::<G, M, RW, S, C>::new())
            .then(Commit::<G, M, RW, S, C>::new(
                git_config.commit_title_template().to_string(),
                git_config.changes_in_body(),
            ))
            .then(Tags::<G, M, RW, S, C>::new(
                git_config.tag_format(),
                use_crate_prefix,
            ))
            .then(UpdateState::<G, M, RW, S, C>::new())
            .build();

        let saga_context = self.create_saga_context(context.project.root());
        saga.execute(&saga_context, saga_data).map_err(Into::into)
    }

    fn create_saga_context(&self, project_root: &Path) -> ReleaseSagaContext<G, M, RW, S, C> {
        ReleaseSagaContext::new(
            project_root.to_path_buf(),
            Arc::clone(&self.git_provider),
            Arc::clone(&self.manifest_writer),
            Arc::clone(&self.changeset_io),
            Arc::clone(&self.release_state_io),
            Arc::clone(&self.changelog_writer),
        )
    }
}

#[cfg(test)]
impl<P, RW, M, C, G, S> ReleaseOperation<P, RW, M, C, G, S> {
    pub(crate) fn manifest_writer(&self) -> &M {
        &self.manifest_writer
    }

    pub(crate) fn git_provider(&self) -> &G {
        &self.git_provider
    }
}

fn populate_dependency_update_entries(
    releases: &[PackageVersion],
    graph: &WorkspaceDependencyGraph,
    template: &str,
    aggregator: &mut ChangesetAggregator,
) {
    for release in releases.iter().filter(|r| r.auto_bumped()) {
        let direct_deps = graph.direct_dependencies(release.name());
        let upgraded_deps: Vec<(String, semver::Version)> = releases
            .iter()
            .filter(|r| direct_deps.contains(r.name().as_str()))
            .map(|r| (r.name().clone(), r.new_version().clone()))
            .collect();
        if !upgraded_deps.is_empty() {
            aggregator.add_dependency_update_entries(release.name(), &upgraded_deps, template);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mocks::{
        MockChangelogWriter, MockChangesetReader, MockGitProvider, MockManifestWriter,
        MockProjectProvider, MockReleaseStateIO, make_changeset,
    };
    use crate::operations::ReleaseInputBuilder;
    use changeset_core::{BumpType, PrereleaseSpec};

    fn default_input() -> ReleaseInput {
        ReleaseInputBuilder::default()
            .dry_run(true)
            .no_commit(true)
            .no_tags(true)
            .keep_changesets(true)
            .build()
            .expect("all fields have defaults")
    }

    fn make_operation<P, RW, M>(
        project_provider: P,
        changeset_io: RW,
        manifest_writer: M,
    ) -> ReleaseOperation<P, RW, M, MockChangelogWriter, MockGitProvider, MockReleaseStateIO>
    where
        P: ProjectProvider + DependencyGraphProvider,
        RW: ChangesetReader + ChangesetWriter + Send + Sync + 'static,
        M: FullManifestWriter + Send + Sync + 'static,
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

        assert_eq!(output.planned_releases().len(), 1);
        let release = &output.planned_releases()[0];
        assert_eq!(release.name(), "my-crate");
        assert_eq!(release.current_version().to_string(), "1.0.0");
        assert_eq!(release.new_version().to_string(), "1.0.1");
        assert_eq!(release.bump_type(), BumpType::Patch);
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

        assert_eq!(output.planned_releases().len(), 1);
        let release = &output.planned_releases()[0];
        assert_eq!(release.new_version().to_string(), "1.3.0");
        assert_eq!(release.bump_type(), BumpType::Minor);
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

        assert_eq!(output.planned_releases().len(), 2);
        assert!(output.unchanged_packages().is_empty());

        let crate_a = output
            .planned_releases()
            .iter()
            .find(|r| r.name() == "crate-a")
            .expect("crate-a should be in releases");
        assert_eq!(crate_a.new_version().to_string(), "1.1.0");

        let crate_b = output
            .planned_releases()
            .iter()
            .find(|r| r.name() == "crate-b")
            .expect("crate-b should be in releases");
        assert_eq!(crate_b.new_version().to_string(), "3.0.0");
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

        assert_eq!(output.planned_releases().len(), 1);
        assert_eq!(output.unchanged_packages().len(), 2);
        assert!(output.unchanged_packages().contains(&"crate-b".to_string()));
        assert!(output.unchanged_packages().contains(&"crate-c".to_string()));
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

        assert_eq!(output.changesets_consumed().len(), 2);
    }

    #[test]
    fn returns_executed_when_not_dry_run() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset = make_changeset("my-crate", BumpType::Patch, "Fix");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/changesets/fix.md"), changeset);
        let manifest_writer = MockManifestWriter::new();

        let operation = make_operation(project_provider, changeset_reader, manifest_writer);
        let input = ReleaseInputBuilder::default()
            .no_commit(true)
            .no_tags(true)
            .keep_changesets(true)
            .build()
            .expect("all fields have defaults");

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
        let input = ReleaseInputBuilder::default()
            .no_commit(true)
            .no_tags(true)
            .keep_changesets(true)
            .build()
            .expect("all fields have defaults");

        let ReleaseOutcome::Executed(output) = operation
            .execute(Path::new("/any"), &input)
            .expect("execute failed")
        else {
            panic!("expected Executed outcome");
        };

        assert_eq!(output.planned_releases().len(), 1);
        assert_eq!(
            output.planned_releases()[0].new_version().to_string(),
            "1.1.0"
        );

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
        let input = ReleaseInputBuilder::default()
            .no_commit(true)
            .no_tags(true)
            .keep_changesets(true)
            .build()
            .expect("all fields have defaults");

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
        let input = ReleaseInputBuilder::default()
            .convert_inherited(true)
            .no_commit(true)
            .no_tags(true)
            .keep_changesets(true)
            .build()
            .expect("all fields have defaults");

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
        let input = ReleaseInputBuilder::default()
            .convert_inherited(true)
            .no_commit(true)
            .no_tags(true)
            .keep_changesets(true)
            .build()
            .expect("all fields have defaults");

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
        let input = ReleaseInputBuilder::default()
            .no_tags(true)
            .keep_changesets(true)
            .build()
            .expect("all fields have defaults");

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
        let input = ReleaseInputBuilder::default()
            .no_commit(true)
            .no_tags(true)
            .keep_changesets(true)
            .build()
            .expect("all fields have defaults");

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
        let input = ReleaseInputBuilder::default()
            .dry_run(true)
            .build()
            .expect("all fields have defaults");

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
        let input = ReleaseInputBuilder::default()
            .no_tags(true)
            .keep_changesets(true)
            .build()
            .expect("all fields have defaults");

        let ReleaseOutcome::Executed(output) = operation
            .execute(Path::new("/any"), &input)
            .expect("execute failed")
        else {
            panic!("expected Executed outcome");
        };

        let git_result = output.git_result().expect("should have git result");
        let commit = git_result.commit().expect("should have commit");
        assert!(commit.message().contains("my-crate@v1.1.0"));
        assert!(commit.message().contains("my-crate 1.0.0 -> 1.1.0"));
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
        let input = ReleaseInputBuilder::default()
            .keep_changesets(true)
            .build()
            .expect("all fields have defaults");

        let ReleaseOutcome::Executed(output) = operation
            .execute(Path::new("/any"), &input)
            .expect("execute failed")
        else {
            panic!("expected Executed outcome");
        };

        let git_result = output.git_result().expect("should have git result");
        assert_eq!(git_result.tags_created().len(), 2);

        let tag_names: Vec<_> = git_result
            .tags_created()
            .iter()
            .map(|t| t.name().as_str())
            .collect();
        assert!(tag_names.contains(&"crate-a@v1.0.1"));
        assert!(tag_names.contains(&"crate-b@v2.0.1"));
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
        let input = ReleaseInputBuilder::default()
            .no_tags(true)
            .keep_changesets(true)
            .build()
            .expect("all fields have defaults");

        let ReleaseOutcome::Executed(output) = operation
            .execute(Path::new("/any"), &input)
            .expect("execute failed")
        else {
            panic!("expected Executed outcome");
        };

        let git_result = output.git_result().expect("should have git result");
        assert!(git_result.tags_created().is_empty());
        assert!(git_result.commit().is_some());
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
        let input = ReleaseInputBuilder::default()
            .keep_changesets(true)
            .build()
            .expect("all fields have defaults");

        let ReleaseOutcome::Executed(output) = operation
            .execute(Path::new("/any"), &input)
            .expect("execute failed")
        else {
            panic!("expected Executed outcome");
        };

        let git_result = output.git_result().expect("should have git result");
        assert_eq!(git_result.tags_created().len(), 1);
        assert_eq!(
            git_result.tags_created()[0].name(),
            "v1.0.1",
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
        let input = ReleaseInputBuilder::default()
            .no_commit(true)
            .no_tags(true)
            .build()
            .expect("all fields have defaults");

        let ReleaseOutcome::Executed(output) = operation
            .execute(Path::new("/any"), &input)
            .expect("execute failed")
        else {
            panic!("expected Executed outcome");
        };

        let git_result = output.git_result().expect("should have git result");
        assert_eq!(git_result.changesets_deleted().len(), 1);
        assert_eq!(
            git_result.changesets_deleted()[0],
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
        let input = ReleaseInputBuilder::default()
            .no_commit(true)
            .no_tags(true)
            .keep_changesets(true)
            .build()
            .expect("all fields have defaults");

        let ReleaseOutcome::Executed(output) = operation
            .execute(Path::new("/any"), &input)
            .expect("execute failed")
        else {
            panic!("expected Executed outcome");
        };

        let git_result = output.git_result().expect("should have git result");
        assert!(
            git_result.changesets_deleted().is_empty(),
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
        let input = ReleaseInputBuilder::default()
            .no_tags(true)
            .build()
            .expect("all fields have defaults");

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
        let input = ReleaseInputBuilder::default()
            .no_tags(true)
            .keep_changesets(true)
            .build()
            .expect("all fields have defaults");

        let ReleaseOutcome::Executed(output) = operation
            .execute(Path::new("/any"), &input)
            .expect("execute failed")
        else {
            panic!("expected Executed outcome");
        };

        let git_result = output.git_result().expect("should have git result");
        let commit = git_result.commit().expect("should have commit");
        assert!(
            !commit.message().contains('\n'),
            "commit message should be title-only without newlines, got: {}",
            commit.message()
        );
        assert!(
            commit.message().contains("my-crate@v1.1.0"),
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
        let input = ReleaseInputBuilder::default()
            .no_commit(true)
            .no_tags(true)
            .keep_changesets(true)
            .global_prerelease(Some(PrereleaseSpec::Alpha))
            .build()
            .expect("all fields have defaults");

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
        let input = ReleaseInputBuilder::default()
            .no_commit(true)
            .no_tags(true)
            .keep_changesets(true)
            .global_prerelease(Some(PrereleaseSpec::Alpha))
            .build()
            .expect("all fields have defaults");

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
        let input = ReleaseInputBuilder::default()
            .no_commit(true)
            .no_tags(true)
            .keep_changesets(true)
            .force(true)
            .global_prerelease(Some(PrereleaseSpec::Alpha))
            .build()
            .expect("all fields have defaults");

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
        let input = ReleaseInputBuilder::default()
            .no_commit(true)
            .no_tags(true)
            .keep_changesets(true)
            .build()
            .expect("all fields have defaults");

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
        let input = ReleaseInputBuilder::default()
            .no_commit(true)
            .no_tags(true)
            .keep_changesets(true)
            .build()
            .expect("all fields have defaults");

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("graduation should succeed");

        assert!(matches!(result, ReleaseOutcome::Executed(_)));

        let written = changelog_writer.written_releases();
        assert_eq!(written.len(), 1, "should write one changelog release");

        let (_, release) = &written[0];
        assert_eq!(
            release.entries().len(),
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
        let input = ReleaseInputBuilder::default()
            .no_commit(true)
            .no_tags(true)
            .keep_changesets(true)
            .build()
            .expect("all fields have defaults");

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("release should succeed");

        let ReleaseOutcome::Executed(output) = result else {
            panic!("expected Executed outcome");
        };

        assert_eq!(output.planned_releases().len(), 1);
        assert_eq!(
            output.planned_releases()[0].new_version().to_string(),
            "1.1.0",
            "should apply minor bump from unconsumed changeset only"
        );

        assert_eq!(
            output.changesets_consumed().len(),
            1,
            "only unconsumed changeset should be in consumed list"
        );
        assert!(
            output.changesets_consumed().contains(&unconsumed_path),
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
        let input = ReleaseInputBuilder::default()
            .no_commit(true)
            .no_tags(true)
            .keep_changesets(true)
            .global_prerelease(Some(PrereleaseSpec::Beta))
            .build()
            .expect("all fields have defaults");

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("prerelease with different tag should succeed");

        let ReleaseOutcome::Executed(output) = result else {
            panic!("expected Executed outcome");
        };

        assert_eq!(output.planned_releases().len(), 1);
        assert_eq!(
            output.planned_releases()[0].new_version().to_string(),
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
        let input = ReleaseInputBuilder::default()
            .no_commit(true)
            .no_tags(true)
            .graduate_all(true)
            .build()
            .expect("all fields have defaults");

        let ReleaseOutcome::Executed(output) = operation
            .execute(Path::new("/any"), &input)
            .expect("zero graduation should succeed")
        else {
            panic!("expected Executed outcome");
        };

        assert_eq!(
            output.planned_releases()[0].new_version().to_string(),
            "1.0.0",
            "zero graduation should bump to 1.0.0"
        );

        let git_result = output.git_result().expect("should have git result");
        assert_eq!(
            git_result.changesets_deleted().len(),
            1,
            "zero graduation should delete changesets"
        );
        assert!(
            git_result.changesets_deleted().contains(&changeset_path),
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
        let input = ReleaseInputBuilder::default()
            .no_commit(true)
            .no_tags(true)
            .build()
            .expect("all fields have defaults");

        let ReleaseOutcome::Executed(output) = operation
            .execute(Path::new("/any"), &input)
            .expect("prerelease graduation should succeed")
        else {
            panic!("expected Executed outcome");
        };

        assert_eq!(
            output.planned_releases()[0].new_version().to_string(),
            "1.0.1",
            "prerelease graduation should remove prerelease suffix"
        );

        let git_result = output.git_result().expect("should have git result");
        assert!(
            git_result.changesets_deleted().is_empty(),
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
        let input = ReleaseInputBuilder::default()
            .no_commit(true)
            .no_tags(true)
            .keep_changesets(true)
            .build()
            .expect("all fields have defaults");

        let ReleaseOutcome::Executed(output) = operation
            .execute(Path::new("/any"), &input)
            .expect("release should succeed")
        else {
            panic!("expected Executed outcome");
        };

        assert_eq!(output.planned_releases().len(), 1);
        assert_eq!(
            output.planned_releases()[0].new_version().to_string(),
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
        let input = ReleaseInputBuilder::default()
            .no_commit(true)
            .no_tags(true)
            .keep_changesets(true)
            .global_prerelease(Some(PrereleaseSpec::Beta))
            .build()
            .expect("all fields have defaults");

        let ReleaseOutcome::Executed(output) = operation
            .execute(Path::new("/any"), &input)
            .expect("release should succeed")
        else {
            panic!("expected Executed outcome");
        };

        assert_eq!(output.planned_releases().len(), 1);
        assert_eq!(
            output.planned_releases()[0].new_version().to_string(),
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
        let input = ReleaseInputBuilder::default()
            .no_commit(true)
            .no_tags(true)
            .keep_changesets(true)
            .build()
            .expect("all fields have defaults");

        let ReleaseOutcome::Executed(output) = operation
            .execute(Path::new("/any"), &input)
            .expect("release should succeed")
        else {
            panic!("expected Executed outcome");
        };

        assert_eq!(output.planned_releases().len(), 1);
        assert_eq!(
            output.planned_releases()[0].new_version().to_string(),
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
        let input = ReleaseInputBuilder::default()
            .no_commit(true)
            .no_tags(true)
            .keep_changesets(true)
            .graduate_all(true)
            .build()
            .expect("all fields have defaults");

        let ReleaseOutcome::Executed(output) = operation
            .execute(Path::new("/any"), &input)
            .expect("release should succeed")
        else {
            panic!("expected Executed outcome");
        };

        assert_eq!(output.planned_releases().len(), 1);
        assert_eq!(
            output.planned_releases()[0].new_version().to_string(),
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
        let input = ReleaseInputBuilder::default()
            .no_commit(true)
            .no_tags(true)
            .keep_changesets(true)
            .build()
            .expect("all fields have defaults");

        let ReleaseOutcome::Executed(output) = operation
            .execute(Path::new("/any"), &input)
            .expect("release should succeed")
        else {
            panic!("expected Executed outcome");
        };

        assert_eq!(output.planned_releases().len(), 1);
        assert_eq!(
            output.planned_releases()[0].new_version().to_string(),
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
        let input = ReleaseInputBuilder::default()
            .no_commit(true)
            .no_tags(true)
            .keep_changesets(true)
            .build()
            .expect("all fields have defaults");

        let ReleaseOutcome::Executed(output) = operation
            .execute(Path::new("/any"), &input)
            .expect("graduation should succeed")
        else {
            panic!("expected Executed outcome");
        };

        assert_eq!(output.planned_releases().len(), 1);
        assert_eq!(
            output.planned_releases()[0].new_version().to_string(),
            "1.0.0",
            "should graduate from prerelease to stable"
        );
        assert!(
            changeset_version::is_prerelease(output.planned_releases()[0].current_version()),
            "current version should have been a prerelease"
        );
        assert!(
            !changeset_version::is_prerelease(output.planned_releases()[0].new_version()),
            "new version should be stable"
        );

        let updated_state = release_state_io.get_prerelease_state();
        assert!(
            updated_state.is_none() || !updated_state.expect("state").contains("my-crate"),
            "graduated package should not be in prerelease state"
        );
    }

    #[test]
    fn saga_rollback_restores_manifest_versions_on_commit_failure() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset = make_changeset("my-crate", BumpType::Patch, "Fix a bug");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/changesets/fix.md"), changeset);
        let manifest_writer = MockManifestWriter::new();
        let git_provider = MockGitProvider::new();
        git_provider.set_fail_on_commit(true);

        let operation = ReleaseOperation::new(
            project_provider,
            changeset_reader,
            manifest_writer,
            MockChangelogWriter::new(),
            git_provider,
            MockReleaseStateIO::new(),
        );
        let input = ReleaseInputBuilder::default()
            .no_tags(true)
            .keep_changesets(true)
            .build()
            .expect("all fields have defaults");

        let result = operation.execute(Path::new("/any"), &input);

        assert!(result.is_err(), "release should fail due to commit failure");

        let versions = operation.manifest_writer().written_versions();
        assert!(
            versions.len() >= 2,
            "should have written version twice (update then rollback), got {} writes",
            versions.len()
        );

        let last_write = &versions.last().expect("should have at least one write");
        assert_eq!(
            last_write.1.to_string(),
            "1.0.0",
            "last write should restore original version"
        );
    }

    #[test]
    fn saga_rollback_deletes_tags_on_failure_after_tag_creation() {
        use std::sync::Arc;

        let project_provider =
            MockProjectProvider::workspace(vec![("crate-a", "1.0.0"), ("crate-b", "2.0.0")]);
        let changeset_a = make_changeset("crate-a", BumpType::Patch, "Fix in crate-a");
        let changeset_b = make_changeset("crate-b", BumpType::Minor, "Feature in crate-b");
        let changeset_reader = Arc::new(
            MockChangesetReader::new()
                .with_changeset(PathBuf::from(".changeset/changesets/fix-a.md"), changeset_a)
                .with_changeset(
                    PathBuf::from(".changeset/changesets/feat-b.md"),
                    changeset_b,
                ),
        );
        let manifest_writer = Arc::new(MockManifestWriter::new());
        let git_provider = Arc::new(MockGitProvider::new());

        let operation = ReleaseOperation::new(
            project_provider,
            Arc::clone(&changeset_reader),
            Arc::clone(&manifest_writer),
            MockChangelogWriter::new(),
            Arc::clone(&git_provider),
            Arc::new(MockReleaseStateIO::new()),
        );
        let input = ReleaseInputBuilder::default()
            .keep_changesets(true)
            .build()
            .expect("all fields have defaults");

        let result = operation.execute(Path::new("/any"), &input);
        assert!(result.is_ok(), "release should succeed");

        let tags = git_provider.tags_created();
        assert_eq!(tags.len(), 2, "should create tags for both packages");
    }

    #[test]
    fn saga_rollback_resets_commit_when_tag_creation_fails() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset = make_changeset("my-crate", BumpType::Patch, "Fix a bug");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/changesets/fix.md"), changeset);
        let manifest_writer = MockManifestWriter::new();
        let git_provider = MockGitProvider::new();
        git_provider.set_fail_on_create_tag(true);

        let operation = ReleaseOperation::new(
            project_provider,
            changeset_reader,
            manifest_writer,
            MockChangelogWriter::new(),
            git_provider,
            MockReleaseStateIO::new(),
        );
        let input = ReleaseInputBuilder::default()
            .keep_changesets(true)
            .build()
            .expect("all fields have defaults");

        let result = operation.execute(Path::new("/any"), &input);

        assert!(
            result.is_err(),
            "release should fail due to tag creation failure"
        );

        assert_eq!(
            operation.git_provider().commits().len(),
            1,
            "should have created one commit before failure"
        );

        assert_eq!(
            operation.git_provider().reset_count(),
            1,
            "should have reset the commit during rollback"
        );

        let versions = operation.manifest_writer().written_versions();
        let last_write = versions.last().expect("should have version writes");
        assert_eq!(
            last_write.1.to_string(),
            "1.0.0",
            "manifest version should be restored to original"
        );
    }

    #[test]
    fn auto_bumps_transitive_dependents() {
        let project_provider = MockProjectProvider::workspace(vec![
            ("core", "1.0.0"),
            ("lib", "1.0.0"),
            ("app", "1.0.0"),
        ])
        .with_dependency_edges(vec![("lib", "core"), ("app", "lib")]);

        let changeset = make_changeset("core", BumpType::Minor, "Add feature to core");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/changesets/feature.md"), changeset);
        let manifest_writer = MockManifestWriter::new();

        let operation = make_operation(project_provider, changeset_reader, manifest_writer);

        let result = operation
            .execute(Path::new("/any"), &default_input())
            .expect("execute failed");

        let ReleaseOutcome::DryRun(output) = result else {
            panic!("expected DryRun outcome");
        };

        assert_eq!(output.planned_releases().len(), 3);

        let core_release = output
            .planned_releases()
            .iter()
            .find(|r| r.name() == "core")
            .expect("core should be in releases");
        assert_eq!(core_release.bump_type(), BumpType::Minor);
        assert!(!core_release.auto_bumped());

        let lib_release = output
            .planned_releases()
            .iter()
            .find(|r| r.name() == "lib")
            .expect("lib should be auto-bumped");
        assert_eq!(lib_release.bump_type(), BumpType::Patch);
        assert!(lib_release.auto_bumped());

        let app_release = output
            .planned_releases()
            .iter()
            .find(|r| r.name() == "app")
            .expect("app should be auto-bumped");
        assert_eq!(app_release.bump_type(), BumpType::Patch);
        assert!(app_release.auto_bumped());
    }

    #[test]
    fn explicit_changeset_takes_precedence_over_auto_bump() {
        let project_provider =
            MockProjectProvider::workspace(vec![("core", "1.0.0"), ("lib", "1.0.0")])
                .with_dependency_edges(vec![("lib", "core")]);

        let changeset1 = make_changeset("core", BumpType::Minor, "Add feature to core");
        let changeset2 = make_changeset("lib", BumpType::Patch, "Fix lib");
        let changeset_reader = MockChangesetReader::new().with_changesets(vec![
            (
                PathBuf::from(".changeset/changesets/feature.md"),
                changeset1,
            ),
            (PathBuf::from(".changeset/changesets/fix.md"), changeset2),
        ]);
        let manifest_writer = MockManifestWriter::new();

        let operation = make_operation(project_provider, changeset_reader, manifest_writer);

        let result = operation
            .execute(Path::new("/any"), &default_input())
            .expect("execute failed");

        let ReleaseOutcome::DryRun(output) = result else {
            panic!("expected DryRun outcome");
        };

        assert_eq!(output.planned_releases().len(), 2);

        let lib_release = output
            .planned_releases()
            .iter()
            .find(|r| r.name() == "lib")
            .expect("lib should be in releases");
        assert_eq!(lib_release.bump_type(), BumpType::Patch);
        assert!(
            !lib_release.auto_bumped(),
            "explicit changeset should take precedence over auto-bump"
        );

        let lib_count = output
            .planned_releases()
            .iter()
            .filter(|r| r.name() == "lib")
            .count();
        assert_eq!(lib_count, 1, "lib should appear exactly once");
    }

    #[test]
    fn no_auto_bump_for_single_package_projects() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");

        let changeset = make_changeset("my-crate", BumpType::Minor, "Add feature");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/changesets/feature.md"), changeset);
        let manifest_writer = MockManifestWriter::new();

        let operation = make_operation(project_provider, changeset_reader, manifest_writer);

        let result = operation
            .execute(Path::new("/any"), &default_input())
            .expect("execute failed");

        let ReleaseOutcome::DryRun(output) = result else {
            panic!("expected DryRun outcome");
        };

        assert_eq!(output.planned_releases().len(), 1);
        assert!(!output.planned_releases()[0].auto_bumped());
    }

    #[test]
    fn no_auto_bump_when_no_dependency_edges() {
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

        assert_eq!(
            output.planned_releases().len(),
            1,
            "only the explicitly changed crate should be released"
        );
        assert_eq!(output.planned_releases()[0].name(), "crate-a");
    }

    #[test]
    fn release_with_promote_to_patch_promotes_none_bump() {
        use changeset_core::NoneBumpBehavior;
        use changeset_project::RootChangesetConfig;

        let root_config = RootChangesetConfig::default()
            .with_none_bump_behavior(NoneBumpBehavior::PromoteToPatch);
        let project_provider =
            MockProjectProvider::single_package("my-crate", "1.0.0").with_root_config(root_config);
        let changeset = make_changeset("my-crate", BumpType::None, "Internal refactor");
        let changeset_reader = MockChangesetReader::new().with_changeset(
            PathBuf::from(".changeset/changesets/refactor.md"),
            changeset,
        );
        let manifest_writer = MockManifestWriter::new();

        let operation = make_operation(project_provider, changeset_reader, manifest_writer);

        let result = operation
            .execute(Path::new("/any"), &default_input())
            .expect("execute failed");

        let ReleaseOutcome::DryRun(output) = result else {
            panic!("expected DryRun outcome");
        };

        assert_eq!(output.planned_releases().len(), 1);
        let release = &output.planned_releases()[0];
        assert_eq!(release.name(), "my-crate");
        assert_eq!(release.bump_type(), BumpType::Patch);
        assert_eq!(release.new_version().to_string(), "1.0.1");
    }

    #[test]
    fn release_with_disallow_errors_on_none_bump() {
        use changeset_core::NoneBumpBehavior;
        use changeset_project::RootChangesetConfig;

        let root_config =
            RootChangesetConfig::default().with_none_bump_behavior(NoneBumpBehavior::Disallow);
        let project_provider =
            MockProjectProvider::single_package("my-crate", "1.0.0").with_root_config(root_config);
        let changeset = make_changeset("my-crate", BumpType::None, "Internal refactor");
        let changeset_reader = MockChangesetReader::new().with_changeset(
            PathBuf::from(".changeset/changesets/refactor.md"),
            changeset,
        );
        let manifest_writer = MockManifestWriter::new();

        let operation = make_operation(project_provider, changeset_reader, manifest_writer);

        let result = operation.execute(Path::new("/any"), &default_input());

        assert!(matches!(
            result,
            Err(crate::error::OperationError::NoneBumpDisallowed { .. })
        ));
    }

    #[test]
    fn release_with_allow_excludes_none_bump_from_releases() {
        use changeset_core::NoneBumpBehavior;
        use changeset_project::RootChangesetConfig;

        let root_config =
            RootChangesetConfig::default().with_none_bump_behavior(NoneBumpBehavior::Allow);
        let project_provider =
            MockProjectProvider::single_package("my-crate", "1.0.0").with_root_config(root_config);
        let changeset = make_changeset("my-crate", BumpType::None, "Internal refactor");
        let changeset_reader = MockChangesetReader::new().with_changeset(
            PathBuf::from(".changeset/changesets/refactor.md"),
            changeset,
        );
        let manifest_writer = MockManifestWriter::new();

        let operation = make_operation(project_provider, changeset_reader, manifest_writer);

        let result = operation
            .execute(Path::new("/any"), &default_input())
            .expect("execute failed");

        let ReleaseOutcome::DryRun(output) = result else {
            panic!("expected DryRun outcome");
        };

        assert!(
            output.planned_releases().is_empty(),
            "None bump with Allow should not produce any planned releases"
        );
    }

    #[test]
    fn release_with_promote_handles_mixed_bumps() {
        use changeset_core::NoneBumpBehavior;
        use changeset_project::RootChangesetConfig;

        let root_config = RootChangesetConfig::default()
            .with_none_bump_behavior(NoneBumpBehavior::PromoteToPatch);
        let project_provider =
            MockProjectProvider::workspace(vec![("crate-a", "1.0.0"), ("crate-b", "2.0.0")])
                .with_root_config(root_config);

        let changeset = changeset_core::Changeset::new(
            "mixed change".to_string(),
            vec![
                changeset_core::PackageRelease::new("crate-a".to_string(), BumpType::Patch),
                changeset_core::PackageRelease::new("crate-b".to_string(), BumpType::None),
            ],
            changeset_core::ChangeCategory::Fixed,
        );
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/changesets/mixed.md"), changeset);
        let manifest_writer = MockManifestWriter::new();

        let operation = make_operation(project_provider, changeset_reader, manifest_writer);

        let result = operation
            .execute(Path::new("/any"), &default_input())
            .expect("execute failed");

        let ReleaseOutcome::DryRun(output) = result else {
            panic!("expected DryRun outcome");
        };

        assert_eq!(output.planned_releases().len(), 2);

        let release_a = output
            .planned_releases()
            .iter()
            .find(|r| r.name() == "crate-a")
            .expect("crate-a should be in planned releases");
        assert_eq!(release_a.bump_type(), BumpType::Patch);

        let release_b = output
            .planned_releases()
            .iter()
            .find(|r| r.name() == "crate-b")
            .expect("crate-b should be in planned releases");
        assert_eq!(release_b.bump_type(), BumpType::Patch);
    }
}
