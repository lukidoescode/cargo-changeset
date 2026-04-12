use std::fs;
use std::path::{Path, PathBuf};

use changeset_core::CARGO_MANIFEST_FILENAME;
use changeset_git::DEFAULT_BASE_BRANCH;
use changeset_manifest::{InitConfig, MetadataSection};
use changeset_project::{CargoProject, ProjectKind, RootChangesetConfig};

use derive_builder::Builder;
use gset::Getset;

use crate::Result;
use crate::traits::{
    ChangelogSettingsInput, FilteringSettingsInput, GitSettingsInput, InitInteractionProvider,
    ManifestMetadataWriter, ProjectContext, ProjectProvider, VersionSettingsInput,
};

#[derive(Debug, Default, Builder, Getset)]
#[builder(default)]
pub struct InitInput {
    #[getset(get_copy, vis = "pub")]
    defaults: bool,
    #[getset(get_as_ref, vis = "pub", ty = "Option<&GitSettingsInput>")]
    git_config: Option<GitSettingsInput>,
    #[getset(get_as_ref, vis = "pub", ty = "Option<&ChangelogSettingsInput>")]
    changelog_config: Option<ChangelogSettingsInput>,
    #[getset(get_as_ref, vis = "pub", ty = "Option<&VersionSettingsInput>")]
    version_config: Option<VersionSettingsInput>,
    #[getset(get_as_ref, vis = "pub", ty = "Option<&FilteringSettingsInput>")]
    filtering_config: Option<FilteringSettingsInput>,
}

#[derive(Debug, Getset)]
pub struct InitPlan {
    #[getset(get, vis = "pub")]
    changeset_dir: PathBuf,
    #[getset(get_copy, vis = "pub")]
    dir_exists: bool,
    #[getset(get_copy, vis = "pub")]
    gitkeep_exists: bool,
    #[getset(get_copy, vis = "pub")]
    metadata_section: MetadataSection,
    #[getset(get, vis = "pub")]
    config: InitConfig,
}

impl InitPlan {
    #[must_use]
    pub fn new(
        changeset_dir: PathBuf,
        dir_exists: bool,
        gitkeep_exists: bool,
        metadata_section: MetadataSection,
        config: InitConfig,
    ) -> Self {
        Self {
            changeset_dir,
            dir_exists,
            gitkeep_exists,
            metadata_section,
            config,
        }
    }
}

#[derive(Debug, Getset)]
#[must_use]
pub struct InitOutput {
    #[getset(get, vis = "pub")]
    changeset_dir: PathBuf,
    #[getset(get_copy, vis = "pub")]
    created_dir: bool,
    #[getset(get_copy, vis = "pub")]
    created_gitkeep: bool,
    #[getset(get_copy, vis = "pub")]
    wrote_config: bool,
    #[getset(get_copy, vis = "pub")]
    config_location: Option<MetadataSection>,
}

impl InitOutput {
    pub(crate) fn new(
        changeset_dir: PathBuf,
        created_dir: bool,
        created_gitkeep: bool,
        wrote_config: bool,
        config_location: Option<MetadataSection>,
    ) -> Self {
        Self {
            changeset_dir,
            created_dir,
            created_gitkeep,
            wrote_config,
            config_location,
        }
    }
}

pub struct InitOperation<P, M = (), I = ()> {
    project_provider: P,
    manifest_writer: Option<M>,
    interaction_provider: Option<I>,
}

impl<P> InitOperation<P, (), ()>
where
    P: ProjectProvider,
{
    pub fn new(project_provider: P) -> Self {
        Self {
            project_provider,
            manifest_writer: None,
            interaction_provider: None,
        }
    }

    /// # Errors
    ///
    /// Returns an error if the project cannot be discovered.
    pub fn prepare_simple(&self, start_path: &Path) -> Result<InitPlan> {
        let project = self.project_provider.discover_project(start_path)?;
        let (root_config, _) = self.project_provider.load_configs(&project)?;

        Ok(build_init_plan(
            &project,
            &root_config,
            InitConfig::default(),
        ))
    }

    /// # Errors
    ///
    /// Returns an error if the changeset directory cannot be created.
    pub fn execute_simple_plan(&self, start_path: &Path, plan: &InitPlan) -> Result<InitOutput> {
        let project = self.project_provider.discover_project(start_path)?;
        let (root_config, _) = self.project_provider.load_configs(&project)?;

        let changeset_dir = self
            .project_provider
            .ensure_changeset_dir(&project, &root_config)?;

        let gitkeep_path = changeset_dir.join(".gitkeep");
        if !plan.gitkeep_exists() {
            fs::write(&gitkeep_path, "")?;
        }

        Ok(InitOutput::new(
            changeset_dir,
            !plan.dir_exists(),
            !plan.gitkeep_exists(),
            false,
            None,
        ))
    }

    /// # Errors
    ///
    /// Returns an error if the project cannot be discovered or the changeset
    /// directory cannot be created.
    pub fn execute_simple(&self, start_path: &Path) -> Result<InitOutput> {
        let plan = self.prepare_simple(start_path)?;
        self.execute_simple_plan(start_path, &plan)
    }
}

impl<P, M, I> InitOperation<P, M, I>
where
    P: ProjectProvider,
{
    #[must_use]
    pub fn with_manifest_writer<M2>(self, writer: M2) -> InitOperation<P, M2, I> {
        InitOperation {
            project_provider: self.project_provider,
            manifest_writer: Some(writer),
            interaction_provider: self.interaction_provider,
        }
    }

    #[must_use]
    pub fn with_interaction_provider<I2>(self, provider: I2) -> InitOperation<P, M, I2> {
        InitOperation {
            project_provider: self.project_provider,
            manifest_writer: self.manifest_writer,
            interaction_provider: Some(provider),
        }
    }
}

impl<P, M, I> InitOperation<P, M, I>
where
    P: ProjectProvider,
    M: ManifestMetadataWriter,
    I: InitInteractionProvider,
{
    /// # Errors
    ///
    /// Returns an error if the project cannot be discovered or configuration
    /// cannot be built (e.g., interactive prompts fail).
    pub fn prepare(&self, start_path: &Path, input: &InitInput) -> Result<InitPlan> {
        let project = self.project_provider.discover_project(start_path)?;
        let (root_config, _) = self.project_provider.load_configs(&project)?;

        let context = ProjectContext {
            is_single_package: *project.kind() == ProjectKind::SinglePackage,
        };
        let config = self.build_config(input, context)?;

        Ok(build_init_plan(&project, &root_config, config))
    }

    /// # Errors
    ///
    /// Returns an error if the changeset directory cannot be created or
    /// configuration cannot be written.
    pub fn execute_plan(&self, start_path: &Path, plan: &InitPlan) -> Result<InitOutput> {
        let project = self.project_provider.discover_project(start_path)?;
        let (root_config, _) = self.project_provider.load_configs(&project)?;

        let changeset_dir = self
            .project_provider
            .ensure_changeset_dir(&project, &root_config)?;

        let gitkeep_path = changeset_dir.join(".gitkeep");
        if !plan.gitkeep_exists() {
            fs::write(&gitkeep_path, "")?;
        }

        let wrote_config = if let Some(ref writer) = self.manifest_writer {
            if plan.config().is_empty() {
                false
            } else {
                let manifest_path = project.root().join(CARGO_MANIFEST_FILENAME);
                writer.write_metadata(&manifest_path, plan.metadata_section(), plan.config())?;
                true
            }
        } else {
            false
        };

        Ok(InitOutput::new(
            changeset_dir,
            !plan.dir_exists(),
            !plan.gitkeep_exists(),
            wrote_config,
            if wrote_config {
                Some(plan.metadata_section())
            } else {
                None
            },
        ))
    }

    /// # Errors
    ///
    /// Returns an error if the project cannot be discovered, the changeset
    /// directory cannot be created, or configuration cannot be written.
    pub fn execute(&self, start_path: &Path, input: &InitInput) -> Result<InitOutput> {
        let plan = self.prepare(start_path, input)?;
        self.execute_plan(start_path, &plan)
    }

    fn build_config(&self, input: &InitInput, context: ProjectContext) -> Result<InitConfig> {
        let mut config = build_config_from_input(input, context);

        if config.is_empty()
            && let Some(ref provider) = self.interaction_provider
        {
            let interactive_input = InitInputBuilder::default()
                .git_config(provider.configure_git_settings(context)?)
                .changelog_config(provider.configure_changelog_settings(context)?)
                .version_config(provider.configure_version_settings()?)
                .filtering_config(provider.configure_filtering_settings()?)
                .build()
                .expect("all fields have defaults");
            apply_settings_to_config(&mut config, &interactive_input);
        }

        Ok(config)
    }
}

fn build_init_plan(
    project: &CargoProject,
    root_config: &RootChangesetConfig,
    config: InitConfig,
) -> InitPlan {
    let changeset_dir_path = root_config.changeset_dir();
    let full_changeset_dir = project.root().join(changeset_dir_path);
    let dir_exists = full_changeset_dir.exists();
    let gitkeep_exists = full_changeset_dir.join(".gitkeep").exists();

    let metadata_section = match project.kind() {
        ProjectKind::VirtualWorkspace | ProjectKind::WorkspaceWithRoot => {
            MetadataSection::Workspace
        }
        ProjectKind::SinglePackage => MetadataSection::Package,
    };

    InitPlan::new(
        full_changeset_dir,
        dir_exists,
        gitkeep_exists,
        metadata_section,
        config,
    )
}

/// The tag format default varies by project type:
/// - Single package: `version-only` (e.g., `v1.0.0`)
/// - Workspace: `crate-prefixed` (e.g., `crate-name@1.0.0`)
#[must_use]
pub(crate) fn build_default_config(context: ProjectContext) -> InitConfig {
    let tag_format = if context.is_single_package {
        changeset_manifest::TagFormat::VersionOnly
    } else {
        changeset_manifest::TagFormat::CratePrefixed
    };

    InitConfig {
        commit: Some(true),
        tags: Some(true),
        keep_changesets: Some(false),
        tag_format: Some(tag_format),
        changelog: Some(changeset_manifest::ChangelogLocation::default()),
        comparison_links: Some(changeset_manifest::ComparisonLinks::default()),
        zero_version_behavior: Some(changeset_manifest::ZeroVersionBehavior::default()),
        dependency_bump_changelog_template: Some(String::from(
            "Updated dependency `{dependency}` to v{version}",
        )),
        base_branch: Some(String::from(DEFAULT_BASE_BRANCH)),
        none_bump_behavior: Some(changeset_manifest::NoneBumpBehavior::default()),
        none_bump_promote_message_template: None,
        commit_title_template: Some(String::from("{new-version}")),
        changes_in_body: Some(true),
        comparison_links_template: None,
        ignored_files: None,
    }
}

#[must_use]
pub fn build_config_from_input(input: &InitInput, context: ProjectContext) -> InitConfig {
    if input.defaults() {
        return build_default_config(context);
    }

    let mut config = InitConfig::default();
    apply_settings_to_config(&mut config, input);
    config
}

fn apply_settings_to_config(config: &mut InitConfig, input: &InitInput) {
    if let Some(git) = input.git_config() {
        config.commit = Some(git.commit);
        config.tags = Some(git.tags);
        config.keep_changesets = Some(git.keep_changesets);
        config.tag_format = Some(git.tag_format);
        config.base_branch = Some(git.base_branch.clone());
        config
            .commit_title_template
            .clone_from(&git.commit_title_template);
        config.changes_in_body = git.changes_in_body;
    }

    if let Some(changelog) = input.changelog_config() {
        config.changelog = Some(changelog.changelog);
        config.comparison_links = Some(changelog.comparison_links);
        config
            .comparison_links_template
            .clone_from(&changelog.comparison_links_template);
        config
            .dependency_bump_changelog_template
            .clone_from(&changelog.dependency_bump_changelog_template);
    }

    if let Some(version) = input.version_config() {
        config.zero_version_behavior = version.zero_version_behavior;
        config.none_bump_behavior = version.none_bump_behavior;
        config
            .none_bump_promote_message_template
            .clone_from(&version.none_bump_promote_message_template);
    }

    if let Some(filtering) = input.filtering_config()
        && !filtering.ignored_files.is_empty()
    {
        config.ignored_files = Some(filtering.ignored_files.clone());
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use changeset_manifest::{ChangelogLocation, ComparisonLinks, TagFormat, ZeroVersionBehavior};

    use super::*;
    use crate::mocks::{MockInitInteractionProvider, MockManifestWriter, MockProjectProvider};

    #[test]
    fn returns_changeset_dir_path() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let changeset_dir = dir.path().join(".changeset");
        std::fs::create_dir_all(&changeset_dir).expect("create changeset dir");

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0")
            .with_changeset_dir(changeset_dir.clone());

        let operation = InitOperation::new(project_provider);

        let result = operation
            .execute_simple(Path::new("/any"))
            .expect("InitOperation failed for single-package project");

        assert_eq!(result.changeset_dir(), &changeset_dir);
    }

    #[test]
    fn works_with_workspace_projects() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let changeset_dir = dir.path().join(".changeset");
        std::fs::create_dir_all(&changeset_dir).expect("create changeset dir");

        let project_provider =
            MockProjectProvider::workspace(vec![("crate-a", "1.0.0"), ("crate-b", "2.0.0")])
                .with_changeset_dir(changeset_dir.clone());

        let operation = InitOperation::new(project_provider);

        let result = operation
            .execute_simple(Path::new("/any"))
            .expect("InitOperation failed for workspace project");

        assert!(
            result
                .changeset_dir()
                .to_string_lossy()
                .contains(".changeset")
        );
    }

    #[test]
    fn creates_gitkeep_file() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let changeset_dir = dir.path().join(".changeset");
        std::fs::create_dir_all(&changeset_dir).expect("create changeset dir");

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0")
            .with_changeset_dir(changeset_dir.clone());

        let operation = InitOperation::new(project_provider);

        let result = operation
            .execute_simple(Path::new("/any"))
            .expect("InitOperation failed");

        assert!(result.created_gitkeep());
        assert!(changeset_dir.join(".gitkeep").exists());
    }

    #[test]
    fn creates_gitkeep_even_when_dir_exists() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let changeset_dir = dir.path().join(".changeset");
        std::fs::create_dir_all(&changeset_dir).expect("create changeset dir");

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0")
            .with_changeset_dir(changeset_dir.clone());

        let operation = InitOperation::new(project_provider);
        let result = operation
            .execute_simple(Path::new("/any"))
            .expect("InitOperation failed");

        assert!(!result.created_dir());
        assert!(result.created_gitkeep());
        assert!(changeset_dir.join(".gitkeep").exists());
    }

    #[test]
    fn writes_config_with_defaults() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let changeset_dir = dir.path().join(".changeset");
        std::fs::create_dir_all(&changeset_dir).expect("create changeset dir");

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0")
            .with_changeset_dir(changeset_dir.clone());
        let manifest_writer = Arc::new(MockManifestWriter::new());
        let interaction_provider = Arc::new(MockInitInteractionProvider::new());

        let operation = InitOperation::new(project_provider)
            .with_manifest_writer(Arc::clone(&manifest_writer))
            .with_interaction_provider(Arc::clone(&interaction_provider));

        let input = InitInputBuilder::default()
            .defaults(true)
            .build()
            .expect("all fields have defaults");

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("InitOperation failed");

        assert!(result.wrote_config());
        assert_eq!(result.config_location(), Some(MetadataSection::Package));

        let written = manifest_writer.written_metadata();
        assert_eq!(written.len(), 1);
        let (_, section, config) = &written[0];
        assert_eq!(*section, MetadataSection::Package);
        assert_eq!(config.commit, Some(true));
        assert_eq!(config.tags, Some(true));
        assert_eq!(config.keep_changesets, Some(false));
    }

    #[test]
    fn writes_config_from_input() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let changeset_dir = dir.path().join(".changeset");
        std::fs::create_dir_all(&changeset_dir).expect("create changeset dir");

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0")
            .with_changeset_dir(changeset_dir.clone());
        let manifest_writer = Arc::new(MockManifestWriter::new());
        let interaction_provider = Arc::new(MockInitInteractionProvider::new());

        let operation = InitOperation::new(project_provider)
            .with_manifest_writer(Arc::clone(&manifest_writer))
            .with_interaction_provider(Arc::clone(&interaction_provider));

        let input = InitInputBuilder::default()
            .git_config(Some(GitSettingsInput {
                commit: false,
                tags: true,
                keep_changesets: true,
                tag_format: TagFormat::CratePrefixed,
                base_branch: String::from("main"),
                ..Default::default()
            }))
            .changelog_config(Some(ChangelogSettingsInput {
                changelog: ChangelogLocation::PerPackage,
                comparison_links: ComparisonLinks::Enabled,
                ..Default::default()
            }))
            .version_config(Some(VersionSettingsInput {
                zero_version_behavior: Some(ZeroVersionBehavior::AutoPromoteOnMajor),
                ..Default::default()
            }))
            .build()
            .expect("all fields have defaults");

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("InitOperation failed");

        assert!(result.wrote_config());

        let written = manifest_writer.written_metadata();
        assert_eq!(written.len(), 1);
        let (_, _, config) = &written[0];
        assert_eq!(config.commit, Some(false));
        assert_eq!(config.tags, Some(true));
        assert_eq!(config.keep_changesets, Some(true));
        assert_eq!(config.tag_format, Some(TagFormat::CratePrefixed));
        assert_eq!(config.changelog, Some(ChangelogLocation::PerPackage));
        assert_eq!(config.comparison_links, Some(ComparisonLinks::Enabled));
        assert_eq!(
            config.zero_version_behavior,
            Some(ZeroVersionBehavior::AutoPromoteOnMajor)
        );
    }

    #[test]
    fn writes_partial_config() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let changeset_dir = dir.path().join(".changeset");
        std::fs::create_dir_all(&changeset_dir).expect("create changeset dir");

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0")
            .with_changeset_dir(changeset_dir.clone());
        let manifest_writer = Arc::new(MockManifestWriter::new());
        let interaction_provider = Arc::new(MockInitInteractionProvider::new());

        let operation = InitOperation::new(project_provider)
            .with_manifest_writer(Arc::clone(&manifest_writer))
            .with_interaction_provider(Arc::clone(&interaction_provider));

        let input = InitInputBuilder::default()
            .git_config(Some(GitSettingsInput {
                commit: true,
                tags: false,
                keep_changesets: false,
                tag_format: TagFormat::VersionOnly,
                base_branch: String::from("main"),
                ..Default::default()
            }))
            .build()
            .expect("all fields have defaults");

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("InitOperation failed");

        assert!(result.wrote_config());

        let written = manifest_writer.written_metadata();
        assert_eq!(written.len(), 1);
        let (_, _, config) = &written[0];
        assert_eq!(config.commit, Some(true));
        assert_eq!(config.tags, Some(false));
        assert!(config.changelog.is_none());
        assert!(config.zero_version_behavior.is_none());
    }

    #[test]
    fn interactive_mode_collects_all_groups() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let changeset_dir = dir.path().join(".changeset");
        std::fs::create_dir_all(&changeset_dir).expect("create changeset dir");

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0")
            .with_changeset_dir(changeset_dir.clone());
        let manifest_writer = Arc::new(MockManifestWriter::new());
        let interaction_provider = Arc::new(MockInitInteractionProvider::all_defaults());

        let operation = InitOperation::new(project_provider)
            .with_manifest_writer(Arc::clone(&manifest_writer))
            .with_interaction_provider(Arc::clone(&interaction_provider));

        let input = InitInput::default();

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("InitOperation failed");

        assert!(result.wrote_config());

        let written = manifest_writer.written_metadata();
        assert_eq!(written.len(), 1);
        let (_, _, config) = &written[0];
        assert!(config.commit.is_some());
        assert!(config.tags.is_some());
        assert!(config.changelog.is_some());
        assert!(config.zero_version_behavior.is_some());
    }

    #[test]
    fn interactive_mode_skips_declined_groups() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let changeset_dir = dir.path().join(".changeset");
        std::fs::create_dir_all(&changeset_dir).expect("create changeset dir");

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0")
            .with_changeset_dir(changeset_dir.clone());
        let manifest_writer = Arc::new(MockManifestWriter::new());
        let interaction_provider = Arc::new(
            MockInitInteractionProvider::new()
                .with_git_settings(Some(GitSettingsInput::default()))
                .with_changelog_settings(None)
                .with_version_settings(None),
        );

        let operation = InitOperation::new(project_provider)
            .with_manifest_writer(Arc::clone(&manifest_writer))
            .with_interaction_provider(Arc::clone(&interaction_provider));

        let input = InitInput::default();

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("InitOperation failed");

        assert!(result.wrote_config());

        let written = manifest_writer.written_metadata();
        assert_eq!(written.len(), 1);
        let (_, _, config) = &written[0];
        assert!(config.commit.is_some());
        assert!(config.changelog.is_none());
        assert!(config.zero_version_behavior.is_none());
    }

    #[test]
    fn skips_config_write_when_empty() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let changeset_dir = dir.path().join(".changeset");
        std::fs::create_dir_all(&changeset_dir).expect("create changeset dir");

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0")
            .with_changeset_dir(changeset_dir.clone());
        let manifest_writer = Arc::new(MockManifestWriter::new());
        let interaction_provider = Arc::new(MockInitInteractionProvider::all_skipped());

        let operation = InitOperation::new(project_provider)
            .with_manifest_writer(Arc::clone(&manifest_writer))
            .with_interaction_provider(Arc::clone(&interaction_provider));

        let input = InitInput::default();

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("InitOperation failed");

        assert!(!result.wrote_config());
        assert!(result.config_location().is_none());

        let written = manifest_writer.written_metadata();
        assert!(written.is_empty());
    }

    #[test]
    fn workspace_uses_workspace_metadata() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let changeset_dir = dir.path().join(".changeset");
        std::fs::create_dir_all(&changeset_dir).expect("create changeset dir");

        let project_provider =
            MockProjectProvider::workspace(vec![("crate-a", "1.0.0"), ("crate-b", "2.0.0")])
                .with_changeset_dir(changeset_dir.clone());
        let manifest_writer = Arc::new(MockManifestWriter::new());
        let interaction_provider = Arc::new(MockInitInteractionProvider::new());

        let operation = InitOperation::new(project_provider)
            .with_manifest_writer(Arc::clone(&manifest_writer))
            .with_interaction_provider(Arc::clone(&interaction_provider));

        let input = InitInputBuilder::default()
            .defaults(true)
            .build()
            .expect("all fields have defaults");

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("InitOperation failed");

        assert!(result.wrote_config());
        assert_eq!(result.config_location(), Some(MetadataSection::Workspace));

        let written = manifest_writer.written_metadata();
        assert_eq!(written.len(), 1);
        let (_, section, _) = &written[0];
        assert_eq!(*section, MetadataSection::Workspace);
    }

    #[test]
    fn single_package_uses_package_metadata() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let changeset_dir = dir.path().join(".changeset");
        std::fs::create_dir_all(&changeset_dir).expect("create changeset dir");

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0")
            .with_changeset_dir(changeset_dir.clone());
        let manifest_writer = Arc::new(MockManifestWriter::new());
        let interaction_provider = Arc::new(MockInitInteractionProvider::new());

        let operation = InitOperation::new(project_provider)
            .with_manifest_writer(Arc::clone(&manifest_writer))
            .with_interaction_provider(Arc::clone(&interaction_provider));

        let input = InitInputBuilder::default()
            .defaults(true)
            .build()
            .expect("all fields have defaults");

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("InitOperation failed");

        assert!(result.wrote_config());
        assert_eq!(result.config_location(), Some(MetadataSection::Package));

        let written = manifest_writer.written_metadata();
        assert_eq!(written.len(), 1);
        let (_, section, _) = &written[0];
        assert_eq!(*section, MetadataSection::Package);
    }

    #[test]
    fn default_config_includes_dependency_bump_changelog_template() {
        let context = ProjectContext {
            is_single_package: true,
        };
        let config = build_default_config(context);
        assert_eq!(
            config.dependency_bump_changelog_template,
            Some("Updated dependency `{dependency}` to v{version}".to_string())
        );
    }

    #[test]
    fn default_config_includes_none_bump_behavior() {
        use changeset_manifest::NoneBumpBehavior;

        let context = ProjectContext {
            is_single_package: true,
        };
        let config = build_default_config(context);
        assert_eq!(config.none_bump_behavior, Some(NoneBumpBehavior::default()));
        assert!(config.none_bump_promote_message_template.is_none());
    }

    #[test]
    fn none_bump_fields_propagate_from_version_settings_input() {
        use changeset_manifest::NoneBumpBehavior;

        let context = ProjectContext {
            is_single_package: true,
        };

        let input = InitInputBuilder::default()
            .version_config(Some(VersionSettingsInput {
                zero_version_behavior: Some(ZeroVersionBehavior::default()),
                none_bump_behavior: Some(NoneBumpBehavior::Disallow),
                none_bump_promote_message_template: Some("Custom message".to_string()),
            }))
            .build()
            .expect("all fields have defaults");

        let config = build_config_from_input(&input, context);
        assert_eq!(config.none_bump_behavior, Some(NoneBumpBehavior::Disallow));
        assert_eq!(
            config.none_bump_promote_message_template,
            Some("Custom message".to_string())
        );
    }

    #[test]
    fn none_bump_fields_default_when_no_version_config() {
        let context = ProjectContext {
            is_single_package: true,
        };

        let input = InitInput::default();

        let config = build_config_from_input(&input, context);
        assert!(config.none_bump_behavior.is_none());
        assert!(config.none_bump_promote_message_template.is_none());
    }

    #[test]
    fn commit_title_template_propagates_from_git_settings() {
        let context = ProjectContext {
            is_single_package: true,
        };

        let input = InitInputBuilder::default()
            .git_config(Some(GitSettingsInput {
                commit_title_template: Some("Release {new-version}".to_string()),
                ..Default::default()
            }))
            .build()
            .expect("all fields have defaults");

        let config = build_config_from_input(&input, context);
        assert_eq!(
            config.commit_title_template,
            Some("Release {new-version}".to_string())
        );
    }

    #[test]
    fn changes_in_body_propagates_from_git_settings() {
        let context = ProjectContext {
            is_single_package: true,
        };

        let input = InitInputBuilder::default()
            .git_config(Some(GitSettingsInput {
                changes_in_body: Some(false),
                ..Default::default()
            }))
            .build()
            .expect("all fields have defaults");

        let config = build_config_from_input(&input, context);
        assert_eq!(config.changes_in_body, Some(false));
    }

    #[test]
    fn comparison_links_template_propagates_from_changelog_settings() {
        let context = ProjectContext {
            is_single_package: true,
        };

        let input = InitInputBuilder::default()
            .changelog_config(Some(ChangelogSettingsInput {
                comparison_links_template: Some(
                    "https://github.com/org/repo/compare/{base}...{target}".to_string(),
                ),
                ..Default::default()
            }))
            .build()
            .expect("all fields have defaults");

        let config = build_config_from_input(&input, context);
        assert_eq!(
            config.comparison_links_template,
            Some("https://github.com/org/repo/compare/{base}...{target}".to_string())
        );
    }

    #[test]
    fn dependency_bump_changelog_template_propagates_from_changelog_settings() {
        let context = ProjectContext {
            is_single_package: true,
        };

        let input = InitInputBuilder::default()
            .changelog_config(Some(ChangelogSettingsInput {
                dependency_bump_changelog_template: Some(
                    "Updated `{dependency}` to v{version}".to_string(),
                ),
                ..Default::default()
            }))
            .build()
            .expect("all fields have defaults");

        let config = build_config_from_input(&input, context);
        assert_eq!(
            config.dependency_bump_changelog_template,
            Some("Updated `{dependency}` to v{version}".to_string())
        );
    }

    #[test]
    fn filtering_config_propagates_ignored_files() {
        let context = ProjectContext {
            is_single_package: true,
        };

        let input = InitInputBuilder::default()
            .filtering_config(Some(FilteringSettingsInput {
                ignored_files: vec!["*.lock".to_string(), "docs/**".to_string()],
            }))
            .build()
            .expect("all fields have defaults");

        let config = build_config_from_input(&input, context);
        assert_eq!(
            config.ignored_files,
            Some(vec!["*.lock".to_string(), "docs/**".to_string()])
        );
    }

    #[test]
    fn filtering_config_skips_empty_ignored_files() {
        let context = ProjectContext {
            is_single_package: true,
        };

        let input = InitInputBuilder::default()
            .filtering_config(Some(FilteringSettingsInput {
                ignored_files: vec![],
            }))
            .build()
            .expect("all fields have defaults");

        let config = build_config_from_input(&input, context);
        assert!(config.ignored_files.is_none());
    }

    #[test]
    fn interactive_mode_collects_filtering_settings() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let changeset_dir = dir.path().join(".changeset");
        std::fs::create_dir_all(&changeset_dir).expect("create changeset dir");

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0")
            .with_changeset_dir(changeset_dir.clone());
        let manifest_writer = Arc::new(MockManifestWriter::new());
        let interaction_provider = Arc::new(
            MockInitInteractionProvider::new()
                .with_git_settings(Some(GitSettingsInput::default()))
                .with_changelog_settings(None)
                .with_version_settings(None)
                .with_filtering_settings(Some(FilteringSettingsInput {
                    ignored_files: vec!["*.lock".to_string()],
                })),
        );

        let operation = InitOperation::new(project_provider)
            .with_manifest_writer(Arc::clone(&manifest_writer))
            .with_interaction_provider(Arc::clone(&interaction_provider));

        let input = InitInput::default();

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("InitOperation failed");

        assert!(result.wrote_config());

        let written = manifest_writer.written_metadata();
        assert_eq!(written.len(), 1);
        let (_, _, config) = &written[0];
        assert_eq!(config.ignored_files, Some(vec!["*.lock".to_string()]));
    }

    #[test]
    fn base_branch_propagates_from_git_settings_input() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let changeset_dir = dir.path().join(".changeset");
        std::fs::create_dir_all(&changeset_dir).expect("create changeset dir");

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0")
            .with_changeset_dir(changeset_dir.clone());
        let manifest_writer = Arc::new(MockManifestWriter::new());
        let interaction_provider = Arc::new(MockInitInteractionProvider::new());

        let operation = InitOperation::new(project_provider)
            .with_manifest_writer(Arc::clone(&manifest_writer))
            .with_interaction_provider(Arc::clone(&interaction_provider));

        let input = InitInputBuilder::default()
            .git_config(Some(GitSettingsInput {
                commit: true,
                tags: true,
                keep_changesets: false,
                tag_format: TagFormat::VersionOnly,
                base_branch: String::from("develop"),
                ..Default::default()
            }))
            .build()
            .expect("all fields have defaults");

        let _ = operation
            .execute(Path::new("/any"), &input)
            .expect("InitOperation failed");

        let written = manifest_writer.written_metadata();
        assert_eq!(written.len(), 1);
        let (_, _, config) = &written[0];
        assert_eq!(config.base_branch, Some(String::from("develop")));
    }
}
