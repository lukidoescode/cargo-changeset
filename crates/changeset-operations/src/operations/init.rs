use std::fs;
use std::path::{Path, PathBuf};

use changeset_manifest::{InitConfig, MetadataSection};
use changeset_project::{CargoProject, ProjectKind, RootChangesetConfig};

use crate::Result;
use crate::traits::{
    ChangelogSettingsInput, GitSettingsInput, InitInteractionProvider, ManifestWriter,
    ProjectContext, ProjectProvider, VersionSettingsInput,
};

/// Input for the init operation.
///
/// Configuration sources have the following precedence (highest to lowest):
/// 1. `defaults: true` - Uses all default values, ignores other fields
/// 2. Explicit `git_config`, `changelog_config`, `version_config` fields
/// 3. Interactive prompts via `InitInteractionProvider` (only if no explicit config)
#[derive(Debug, Default)]
pub struct InitInput {
    pub defaults: bool,
    pub git_config: Option<GitSettingsInput>,
    pub changelog_config: Option<ChangelogSettingsInput>,
    pub version_config: Option<VersionSettingsInput>,
}

/// A preview of what the init operation will do, without performing any changes.
#[derive(Debug)]
pub struct InitPlan {
    pub changeset_dir: PathBuf,
    pub dir_exists: bool,
    pub gitkeep_exists: bool,
    pub metadata_section: MetadataSection,
    pub config: InitConfig,
}

#[derive(Debug)]
#[must_use]
pub struct InitOutput {
    pub changeset_dir: PathBuf,
    pub created_dir: bool,
    pub created_gitkeep: bool,
    pub wrote_config: bool,
    pub config_location: Option<MetadataSection>,
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
    M: ManifestWriter,
    I: InitInteractionProvider,
{
    /// Prepares an initialization plan by collecting all configuration without
    /// performing any file system operations.
    ///
    /// # Errors
    ///
    /// Returns an error if the project cannot be discovered or configuration
    /// cannot be built (e.g., interactive prompts fail).
    pub fn prepare(&self, start_path: &Path, input: &InitInput) -> Result<InitPlan> {
        let project = self.project_provider.discover_project(start_path)?;
        let (root_config, _) = self.project_provider.load_configs(&project)?;

        let context = ProjectContext {
            is_single_package: project.kind == ProjectKind::SinglePackage,
        };
        let config = self.build_config(input, context)?;

        Ok(build_init_plan(&project, &root_config, config))
    }

    /// Executes the init operation using a pre-built plan.
    ///
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
        if !plan.gitkeep_exists {
            fs::write(&gitkeep_path, "")?;
        }

        let wrote_config = if let Some(ref writer) = self.manifest_writer {
            if plan.config.is_empty() {
                false
            } else {
                let manifest_path = project.root.join("Cargo.toml");
                writer.write_metadata(&manifest_path, plan.metadata_section, &plan.config)?;
                true
            }
        } else {
            false
        };

        Ok(InitOutput {
            changeset_dir,
            created_dir: !plan.dir_exists,
            created_gitkeep: !plan.gitkeep_exists,
            wrote_config,
            config_location: if wrote_config {
                Some(plan.metadata_section)
            } else {
                None
            },
        })
    }

    /// Executes the full init operation (prepare + execute).
    ///
    /// # Errors
    ///
    /// Returns an error if the project cannot be discovered, the changeset
    /// directory cannot be created, or configuration cannot be written.
    pub fn execute(&self, start_path: &Path, input: &InitInput) -> Result<InitOutput> {
        let plan = self.prepare(start_path, input)?;
        self.execute_plan(start_path, &plan)
    }

    fn build_config(&self, input: &InitInput, context: ProjectContext) -> Result<InitConfig> {
        if input.defaults {
            return Ok(build_default_config(context));
        }

        let mut config = InitConfig::default();

        if let Some(ref git) = input.git_config {
            config.commit = Some(git.commit);
            config.tags = Some(git.tags);
            config.keep_changesets = Some(git.keep_changesets);
            config.tag_format = Some(git.tag_format);
        }

        if let Some(ref changelog) = input.changelog_config {
            config.changelog = Some(changelog.changelog);
            config.comparison_links = Some(changelog.comparison_links);
        }

        if let Some(ref version) = input.version_config {
            config.zero_version_behavior = Some(version.zero_version_behavior);
        }

        if config.is_empty() {
            if let Some(ref provider) = self.interaction_provider {
                if let Some(git) = provider.configure_git_settings(context)? {
                    config.commit = Some(git.commit);
                    config.tags = Some(git.tags);
                    config.keep_changesets = Some(git.keep_changesets);
                    config.tag_format = Some(git.tag_format);
                }

                if let Some(changelog) = provider.configure_changelog_settings(context)? {
                    config.changelog = Some(changelog.changelog);
                    config.comparison_links = Some(changelog.comparison_links);
                }

                if let Some(version) = provider.configure_version_settings()? {
                    config.zero_version_behavior = Some(version.zero_version_behavior);
                }
            }
        }

        Ok(config)
    }
}

/// Builds an `InitPlan` from project information and configuration.
fn build_init_plan(
    project: &CargoProject,
    root_config: &RootChangesetConfig,
    config: InitConfig,
) -> InitPlan {
    let changeset_dir_path = root_config.changeset_dir();
    let full_changeset_dir = project.root.join(changeset_dir_path);
    let dir_exists = full_changeset_dir.exists();
    let gitkeep_exists = full_changeset_dir.join(".gitkeep").exists();

    let metadata_section = match project.kind {
        ProjectKind::VirtualWorkspace | ProjectKind::WorkspaceWithRoot => {
            MetadataSection::Workspace
        }
        ProjectKind::SinglePackage => MetadataSection::Package,
    };

    InitPlan {
        changeset_dir: full_changeset_dir,
        dir_exists,
        gitkeep_exists,
        metadata_section,
        config,
    }
}

/// Builds the default configuration with all options set to their defaults.
///
/// The tag format default varies by project type:
/// - Single package: `version-only` (e.g., `v1.0.0`)
/// - Workspace: `crate-prefixed` (e.g., `crate-name@1.0.0`)
#[must_use]
pub fn build_default_config(context: ProjectContext) -> InitConfig {
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
    }
}

/// Builds an `InitConfig` from the provided input settings.
#[must_use]
pub fn build_config_from_input(input: &InitInput, context: ProjectContext) -> InitConfig {
    if input.defaults {
        return build_default_config(context);
    }

    let mut config = InitConfig::default();

    if let Some(ref git) = input.git_config {
        config.commit = Some(git.commit);
        config.tags = Some(git.tags);
        config.keep_changesets = Some(git.keep_changesets);
        config.tag_format = Some(git.tag_format);
    }

    if let Some(ref changelog) = input.changelog_config {
        config.changelog = Some(changelog.changelog);
        config.comparison_links = Some(changelog.comparison_links);
    }

    if let Some(ref version) = input.version_config {
        config.zero_version_behavior = Some(version.zero_version_behavior);
    }

    config
}

impl<P> InitOperation<P, (), ()>
where
    P: ProjectProvider,
{
    /// Prepares a simple initialization plan without configuration.
    ///
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

    /// Executes the simple init operation using a pre-built plan.
    ///
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
        if !plan.gitkeep_exists {
            fs::write(&gitkeep_path, "")?;
        }

        Ok(InitOutput {
            changeset_dir,
            created_dir: !plan.dir_exists,
            created_gitkeep: !plan.gitkeep_exists,
            wrote_config: false,
            config_location: None,
        })
    }

    /// Simple execute method for backward compatibility when no manifest writer
    /// or interaction provider is configured.
    ///
    /// # Errors
    ///
    /// Returns an error if the project cannot be discovered or the changeset
    /// directory cannot be created.
    pub fn execute_simple(&self, start_path: &Path) -> Result<InitOutput> {
        let plan = self.prepare_simple(start_path)?;
        self.execute_simple_plan(start_path, &plan)
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

        assert_eq!(result.changeset_dir, changeset_dir);
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
                .changeset_dir
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

        assert!(result.created_gitkeep);
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

        assert!(!result.created_dir);
        assert!(result.created_gitkeep);
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

        let input = InitInput {
            defaults: true,
            ..Default::default()
        };

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("InitOperation failed");

        assert!(result.wrote_config);
        assert_eq!(result.config_location, Some(MetadataSection::Package));

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

        let input = InitInput {
            defaults: false,
            git_config: Some(GitSettingsInput {
                commit: false,
                tags: true,
                keep_changesets: true,
                tag_format: TagFormat::CratePrefixed,
            }),
            changelog_config: Some(ChangelogSettingsInput {
                changelog: ChangelogLocation::PerPackage,
                comparison_links: ComparisonLinks::Enabled,
            }),
            version_config: Some(VersionSettingsInput {
                zero_version_behavior: ZeroVersionBehavior::AutoPromoteOnMajor,
            }),
        };

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("InitOperation failed");

        assert!(result.wrote_config);

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

        let input = InitInput {
            defaults: false,
            git_config: Some(GitSettingsInput {
                commit: true,
                tags: false,
                keep_changesets: false,
                tag_format: TagFormat::VersionOnly,
            }),
            changelog_config: None,
            version_config: None,
        };

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("InitOperation failed");

        assert!(result.wrote_config);

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

        assert!(result.wrote_config);

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

        assert!(result.wrote_config);

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

        assert!(!result.wrote_config);
        assert!(result.config_location.is_none());

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

        let input = InitInput {
            defaults: true,
            ..Default::default()
        };

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("InitOperation failed");

        assert!(result.wrote_config);
        assert_eq!(result.config_location, Some(MetadataSection::Workspace));

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

        let input = InitInput {
            defaults: true,
            ..Default::default()
        };

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("InitOperation failed");

        assert!(result.wrote_config);
        assert_eq!(result.config_location, Some(MetadataSection::Package));

        let written = manifest_writer.written_metadata();
        assert_eq!(written.len(), 1);
        let (_, section, _) = &written[0];
        assert_eq!(*section, MetadataSection::Package);
    }
}
