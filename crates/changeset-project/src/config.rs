use std::collections::HashMap;
use std::path::{Path, PathBuf};

use changeset_changelog::{ChangelogConfig, ChangelogLocation, ComparisonLinksSetting};
use changeset_core::{
    AdditionalPackageDeclaration, CARGO_MANIFEST_FILENAME, NoneBumpBehavior,
    VersionTrackingDependency, ZeroVersionBehavior,
};
use changeset_git::DEFAULT_BASE_BRANCH;
use globset::{Glob, GlobSet, GlobSetBuilder};

use crate::error::ProjectError;
use crate::manifest::{
    CargoManifest, ChangesetMetadata, TagFormatValue, read_manifest, read_package_level_manifest,
};
use crate::project::{CargoProject, ProjectKind};

const DEFAULT_DEPENDENCY_BUMP_CHANGELOG_TEMPLATE: &str =
    "Updated dependency `{dependency}` to v{version}";
const DEFAULT_NONE_BUMP_PROMOTE_MESSAGE_TEMPLATE: &str = "Internal architectural changes";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TagFormat {
    #[default]
    VersionOnly,
    CratePrefixed,
}

#[derive(Debug, Clone, Copy)]
struct GitOperationFlags {
    commit: bool,
    tags: bool,
}

impl Default for GitOperationFlags {
    fn default() -> Self {
        Self {
            commit: true,
            tags: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GitConfig {
    operations: GitOperationFlags,
    keep_changesets: bool,
    tag_format: TagFormat,
    commit_title_template: String,
    changes_in_body: bool,
}

impl GitConfig {
    #[must_use]
    pub fn commit(&self) -> bool {
        self.operations.commit
    }

    #[must_use]
    pub fn tags(&self) -> bool {
        self.operations.tags
    }

    #[must_use]
    pub fn keep_changesets(&self) -> bool {
        self.keep_changesets
    }

    #[must_use]
    pub fn tag_format(&self) -> TagFormat {
        self.tag_format
    }

    #[must_use]
    pub fn commit_title_template(&self) -> &str {
        &self.commit_title_template
    }

    #[must_use]
    pub fn changes_in_body(&self) -> bool {
        self.changes_in_body
    }

    #[cfg(any(test, feature = "testing"))]
    #[must_use]
    pub fn with_changes_in_body(mut self, changes_in_body: bool) -> Self {
        self.changes_in_body = changes_in_body;
        self
    }
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            operations: GitOperationFlags::default(),
            keep_changesets: false,
            tag_format: TagFormat::default(),
            commit_title_template: String::from("{new-version}"),
            changes_in_body: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RootChangesetConfig {
    ignored_files: GlobSet,
    changeset_dir: PathBuf,
    changelog_config: ChangelogConfig,
    git_config: GitConfig,
    zero_version_behavior: ZeroVersionBehavior,
    dependency_bump_changelog_template: String,
    base_branch: String,
    none_bump_behavior: changeset_core::NoneBumpBehavior,
    none_bump_promote_message_template: String,
    additional_packages: Vec<changeset_core::AdditionalPackageDeclaration>,
}

impl RootChangesetConfig {
    #[must_use]
    pub fn ignored_files(&self) -> &GlobSet {
        &self.ignored_files
    }

    #[must_use]
    pub fn is_ignored(&self, path: &Path) -> bool {
        self.ignored_files.is_match(path)
    }

    #[must_use]
    pub fn changeset_dir(&self) -> &Path {
        &self.changeset_dir
    }

    #[must_use]
    pub fn changelog_config(&self) -> &ChangelogConfig {
        &self.changelog_config
    }

    #[must_use]
    pub fn git_config(&self) -> &GitConfig {
        &self.git_config
    }

    #[must_use]
    pub fn zero_version_behavior(&self) -> ZeroVersionBehavior {
        self.zero_version_behavior
    }

    #[must_use]
    pub fn dependency_bump_changelog_template(&self) -> &str {
        &self.dependency_bump_changelog_template
    }

    #[must_use]
    pub fn base_branch(&self) -> &str {
        &self.base_branch
    }

    #[must_use]
    pub fn none_bump_behavior(&self) -> changeset_core::NoneBumpBehavior {
        self.none_bump_behavior
    }

    #[must_use]
    pub fn none_bump_promote_message_template(&self) -> &str {
        &self.none_bump_promote_message_template
    }

    #[must_use]
    pub fn additional_packages(&self) -> &[changeset_core::AdditionalPackageDeclaration] {
        &self.additional_packages
    }

    #[cfg(any(test, feature = "testing"))]
    #[must_use]
    pub fn with_git_config(mut self, git_config: GitConfig) -> Self {
        self.git_config = git_config;
        self
    }

    #[cfg(any(test, feature = "testing"))]
    #[must_use]
    pub fn with_none_bump_behavior(mut self, behavior: changeset_core::NoneBumpBehavior) -> Self {
        self.none_bump_behavior = behavior;
        self
    }

    #[cfg(any(test, feature = "testing"))]
    #[must_use]
    pub fn with_additional_packages(
        mut self,
        packages: Vec<changeset_core::AdditionalPackageDeclaration>,
    ) -> Self {
        self.additional_packages = packages;
        self
    }
}

impl Default for RootChangesetConfig {
    fn default() -> Self {
        Self {
            ignored_files: GlobSet::empty(),
            changeset_dir: PathBuf::from(crate::DEFAULT_CHANGESET_DIR),
            changelog_config: ChangelogConfig::default(),
            git_config: GitConfig::default(),
            zero_version_behavior: ZeroVersionBehavior::default(),
            dependency_bump_changelog_template: String::from(
                DEFAULT_DEPENDENCY_BUMP_CHANGELOG_TEMPLATE,
            ),
            base_branch: String::from(DEFAULT_BASE_BRANCH),
            none_bump_behavior: changeset_core::NoneBumpBehavior::default(),
            none_bump_promote_message_template: String::from(
                DEFAULT_NONE_BUMP_PROMOTE_MESSAGE_TEMPLATE,
            ),
            additional_packages: Vec::new(),
        }
    }
}

#[derive(Debug, Default)]
pub struct PackageChangesetConfig {
    ignored_files: GlobSet,
    additional_package_dependencies: Vec<VersionTrackingDependency>,
}

impl PackageChangesetConfig {
    #[must_use]
    pub fn ignored_files(&self) -> &GlobSet {
        &self.ignored_files
    }

    #[must_use]
    pub fn is_ignored(&self, path: &Path) -> bool {
        self.ignored_files.is_match(path)
    }

    #[must_use]
    pub fn additional_package_dependencies(&self) -> &[VersionTrackingDependency] {
        &self.additional_package_dependencies
    }
}

#[derive(Copy, Clone)]
enum CargoRootConfigType {
    Workspace,
    Package,
}

struct RootConfigFields {
    patterns: Vec<String>,
    changeset_dir: String,
    zero_version_behavior: ZeroVersionBehavior,
    dependency_bump_changelog_template: String,
    base_branch: String,
    none_bump_behavior: NoneBumpBehavior,
    none_bump_promote_message_template: String,
    additional_packages: Vec<AdditionalPackageDeclaration>,
    changelog: Option<ChangelogLocation>,
    comparison_links: Option<ComparisonLinksSetting>,
    comparison_links_template: Option<String>,
    git_metadata: Option<ChangesetMetadata>,
}

/// Parses the root changeset configuration based on project kind.
///
/// For single-package projects, reads from `[package.metadata.changeset]`.
/// For workspaces, reads from `[workspace.metadata.changeset]`.
///
/// # Errors
///
/// Returns `ProjectError` if the manifest cannot be read or parsed, or if glob patterns are invalid.
pub fn parse_root_config(project: &CargoProject) -> Result<RootChangesetConfig, ProjectError> {
    match project.kind() {
        ProjectKind::SinglePackage => {
            parse_cargo_root_config(project.root(), CargoRootConfigType::Package)
        }
        ProjectKind::VirtualWorkspace | ProjectKind::WorkspaceWithRoot => {
            parse_cargo_root_config(project.root(), CargoRootConfigType::Workspace)
        }
    }
}

/// # Errors
///
/// Returns `ProjectError` if the manifest cannot be read or parsed, or if glob patterns are invalid.
pub fn parse_package_config(package_path: &Path) -> Result<PackageChangesetConfig, ProjectError> {
    let manifest_path = package_path.join(CARGO_MANIFEST_FILENAME);
    let manifest = read_package_level_manifest(&manifest_path)?;

    let changeset_metadata = manifest
        .package
        .and_then(|pkg| pkg.metadata)
        .and_then(|meta| meta.changeset);

    let patterns = changeset_metadata
        .as_ref()
        .map(|cs| cs.ignored_files.clone())
        .unwrap_or_default();

    let additional_package_dependencies = changeset_metadata
        .as_ref()
        .map(|cs| cs.additional_package_dependencies.clone())
        .unwrap_or_default();

    let ignored_files = build_glob_set(&patterns)?;

    Ok(PackageChangesetConfig {
        ignored_files,
        additional_package_dependencies,
    })
}

/// # Errors
///
/// Returns an error if any manifest cannot be read or parsed, or if glob patterns are invalid.
pub fn load_changeset_configs(
    project: &CargoProject,
) -> Result<(RootChangesetConfig, HashMap<String, PackageChangesetConfig>), ProjectError> {
    let root_config = parse_root_config(project)?;

    let mut package_configs = HashMap::new();
    for package in project.packages() {
        let config = parse_package_config(package.path())?;
        package_configs.insert(package.name().clone(), config);
    }

    Ok((root_config, package_configs))
}

fn build_glob_set(patterns: &[String]) -> Result<GlobSet, ProjectError> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = Glob::new(pattern).map_err(|source| ProjectError::GlobPattern {
            pattern: pattern.clone(),
            source,
        })?;
        builder.add(glob);
    }
    builder.build().map_err(|source| ProjectError::GlobPattern {
        pattern: patterns.join(", "),
        source,
    })
}

fn extract_changeset_metadata(
    manifest: CargoManifest,
    config_type: CargoRootConfigType,
) -> Option<ChangesetMetadata> {
    match config_type {
        CargoRootConfigType::Workspace => manifest
            .workspace
            .and_then(|ws| ws.metadata)
            .and_then(|meta| meta.changeset),
        CargoRootConfigType::Package => manifest
            .package
            .and_then(|pkg| pkg.metadata)
            .and_then(|meta| meta.changeset),
    }
}

fn extract_root_config_fields(metadata: Option<ChangesetMetadata>) -> RootConfigFields {
    RootConfigFields {
        patterns: metadata
            .as_ref()
            .map(|cs| cs.ignored_files.clone())
            .unwrap_or_default(),
        changeset_dir: metadata
            .as_ref()
            .and_then(|cs| cs.changeset_dir.clone())
            .unwrap_or_else(|| crate::DEFAULT_CHANGESET_DIR.to_string()),
        zero_version_behavior: metadata
            .as_ref()
            .and_then(|cs| cs.zero_version_behavior)
            .unwrap_or_default(),
        dependency_bump_changelog_template: metadata
            .as_ref()
            .and_then(|cs| cs.dependency_bump_changelog_template.clone())
            .unwrap_or_else(|| String::from(DEFAULT_DEPENDENCY_BUMP_CHANGELOG_TEMPLATE)),
        base_branch: metadata
            .as_ref()
            .and_then(|cs| cs.base_branch.clone())
            .unwrap_or_else(|| String::from(DEFAULT_BASE_BRANCH)),
        none_bump_behavior: metadata
            .as_ref()
            .and_then(|cs| cs.none_bump_behavior)
            .unwrap_or_default(),
        none_bump_promote_message_template: metadata
            .as_ref()
            .and_then(|cs| cs.none_bump_promote_message_template.clone())
            .unwrap_or_else(|| String::from(DEFAULT_NONE_BUMP_PROMOTE_MESSAGE_TEMPLATE)),
        additional_packages: metadata
            .as_ref()
            .map(|cs| cs.additional_packages.clone())
            .unwrap_or_default(),
        changelog: metadata.as_ref().and_then(|cs| cs.changelog),
        comparison_links: metadata.as_ref().and_then(|cs| cs.comparison_links),
        comparison_links_template: metadata
            .as_ref()
            .and_then(|cs| cs.comparison_links_template.clone()),
        git_metadata: metadata,
    }
}

fn build_changelog_config(
    changelog: Option<ChangelogLocation>,
    comparison_links: Option<ComparisonLinksSetting>,
    comparison_links_template: Option<String>,
) -> ChangelogConfig {
    ChangelogConfig::new(
        changelog.unwrap_or_default(),
        comparison_links.unwrap_or_default(),
        comparison_links_template,
    )
}

fn build_git_config(metadata: Option<&ChangesetMetadata>) -> GitConfig {
    let defaults = GitConfig::default();
    match metadata {
        None => defaults,
        Some(cs) => GitConfig {
            operations: GitOperationFlags {
                commit: cs.commit.unwrap_or(defaults.operations.commit),
                tags: cs.tags.unwrap_or(defaults.operations.tags),
            },
            keep_changesets: cs.keep_changesets.unwrap_or(defaults.keep_changesets),
            tag_format: cs.tag_format.map_or(defaults.tag_format, |tf| match tf {
                TagFormatValue::VersionOnly => TagFormat::VersionOnly,
                TagFormatValue::CratePrefixed => TagFormat::CratePrefixed,
            }),
            commit_title_template: cs
                .commit_title_template
                .clone()
                .unwrap_or(defaults.commit_title_template),
            changes_in_body: cs.changes_in_body.unwrap_or(defaults.changes_in_body),
        },
    }
}

fn parse_cargo_root_config(
    project_root: &Path,
    config_type: CargoRootConfigType,
) -> Result<RootChangesetConfig, ProjectError> {
    let manifest = read_manifest(&project_root.join(CARGO_MANIFEST_FILENAME))?;
    let metadata = extract_changeset_metadata(manifest, config_type);
    let fields = extract_root_config_fields(metadata);

    Ok(RootChangesetConfig {
        ignored_files: build_glob_set(&fields.patterns)?,
        changeset_dir: PathBuf::from(fields.changeset_dir),
        changelog_config: build_changelog_config(
            fields.changelog,
            fields.comparison_links,
            fields.comparison_links_template,
        ),
        git_config: build_git_config(fields.git_metadata.as_ref()),
        zero_version_behavior: fields.zero_version_behavior,
        dependency_bump_changelog_template: fields.dependency_bump_changelog_template,
        base_branch: fields.base_branch,
        none_bump_behavior: fields.none_bump_behavior,
        none_bump_promote_message_template: fields.none_bump_promote_message_template,
        additional_packages: fields.additional_packages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_with_config(toml_content: &str) -> anyhow::Result<TempDir> {
        let dir = TempDir::new()?;
        fs::write(dir.path().join("Cargo.toml"), toml_content)?;
        Ok(dir)
    }

    #[test]
    fn parse_workspace_root_config_with_ignored_files() -> anyhow::Result<()> {
        let toml = r#"
[workspace]
members = ["crates/*"]

[workspace.metadata.changeset]
ignored-files = ["*.md", "docs/**"]
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_cargo_root_config(dir.path(), CargoRootConfigType::Workspace)?;

        assert!(config.is_ignored(Path::new("README.md")));
        assert!(config.is_ignored(Path::new("docs/guide.md")));
        assert!(!config.is_ignored(Path::new("src/lib.rs")));

        Ok(())
    }

    #[test]
    fn parse_workspace_root_config_without_metadata() -> anyhow::Result<()> {
        let toml = r#"
[workspace]
members = ["crates/*"]
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_cargo_root_config(dir.path(), CargoRootConfigType::Workspace)?;

        assert!(!config.is_ignored(Path::new("README.md")));
        assert!(!config.is_ignored(Path::new("src/lib.rs")));

        Ok(())
    }

    #[test]
    fn parse_workspace_root_config_with_custom_changeset_dir() -> anyhow::Result<()> {
        let toml = r#"
[workspace]
members = ["crates/*"]

[workspace.metadata.changeset]
changeset-dir = "changes"
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_cargo_root_config(dir.path(), CargoRootConfigType::Workspace)?;

        assert_eq!(config.changeset_dir(), Path::new("changes"));

        Ok(())
    }

    #[test]
    fn parse_workspace_root_config_default_changeset_dir() -> anyhow::Result<()> {
        let toml = r#"
[workspace]
members = ["crates/*"]
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_cargo_root_config(dir.path(), CargoRootConfigType::Workspace)?;

        assert_eq!(config.changeset_dir(), Path::new(".changeset"));

        Ok(())
    }

    #[test]
    fn parse_package_config_with_ignored_files() -> anyhow::Result<()> {
        let toml = r#"
[package]
name = "my-crate"
version = "0.1.0"

[package.metadata.changeset]
ignored-files = ["benches/**", "examples/**"]
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_package_config(dir.path())?;

        assert!(config.is_ignored(Path::new("benches/bench.rs")));
        assert!(config.is_ignored(Path::new("examples/demo.rs")));
        assert!(!config.is_ignored(Path::new("src/lib.rs")));

        Ok(())
    }

    #[test]
    fn parse_package_config_without_metadata() -> anyhow::Result<()> {
        let toml = r#"
[package]
name = "my-crate"
version = "0.1.0"
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_package_config(dir.path())?;

        assert!(!config.is_ignored(Path::new("benches/bench.rs")));

        Ok(())
    }

    #[test]
    fn parse_single_package_root_config() -> anyhow::Result<()> {
        let toml = r#"
[package]
name = "my-crate"
version = "0.1.0"

[package.metadata.changeset]
ignored-files = ["*.md"]
changeset-dir = "my-changesets"
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_cargo_root_config(dir.path(), CargoRootConfigType::Package)?;

        assert!(config.is_ignored(Path::new("README.md")));
        assert_eq!(config.changeset_dir(), Path::new("my-changesets"));

        Ok(())
    }

    #[test]
    fn invalid_glob_pattern_returns_error() -> anyhow::Result<()> {
        let toml = r#"
[workspace]
members = ["crates/*"]

[workspace.metadata.changeset]
ignored-files = ["[invalid"]
"#;
        let dir = setup_with_config(toml)?;

        let result = parse_cargo_root_config(dir.path(), CargoRootConfigType::Workspace);

        assert!(result.is_err());
        let err = result.expect_err("should fail on invalid glob");
        assert!(matches!(err, ProjectError::GlobPattern { .. }));

        Ok(())
    }

    #[test]
    fn empty_ignored_files_list() -> anyhow::Result<()> {
        let toml = r#"
[workspace]
members = ["crates/*"]

[workspace.metadata.changeset]
ignored-files = []
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_cargo_root_config(dir.path(), CargoRootConfigType::Workspace)?;

        assert!(!config.is_ignored(Path::new("anything.txt")));

        Ok(())
    }

    #[test]
    fn parse_workspace_changelog_config() -> anyhow::Result<()> {
        let toml = r#"
[workspace]
members = ["crates/*"]

[workspace.metadata.changeset]
changelog = "per-package"
comparison-links = "enabled"
comparison-links-template = "https://example.com/{repository}/compare/{base}...{target}"
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_cargo_root_config(dir.path(), CargoRootConfigType::Workspace)?;
        let changelog_config = config.changelog_config();

        assert_eq!(changelog_config.changelog(), ChangelogLocation::PerPackage);
        assert_eq!(
            changelog_config.comparison_links(),
            ComparisonLinksSetting::Enabled
        );
        assert_eq!(
            changelog_config.comparison_links_template(),
            Some("https://example.com/{repository}/compare/{base}...{target}")
        );

        Ok(())
    }

    #[test]
    fn parse_changelog_config_defaults() -> anyhow::Result<()> {
        let toml = r#"
[workspace]
members = ["crates/*"]
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_cargo_root_config(dir.path(), CargoRootConfigType::Workspace)?;
        let changelog_config = config.changelog_config();

        assert_eq!(changelog_config.changelog(), ChangelogLocation::Root);
        assert_eq!(
            changelog_config.comparison_links(),
            ComparisonLinksSetting::Auto
        );
        assert!(changelog_config.comparison_links_template().is_none());

        Ok(())
    }

    #[test]
    fn parse_single_package_changelog_config() -> anyhow::Result<()> {
        let toml = r#"
[package]
name = "my-crate"
version = "0.1.0"

[package.metadata.changeset]
changelog = "root"
comparison-links = "disabled"
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_cargo_root_config(dir.path(), CargoRootConfigType::Package)?;
        let changelog_config = config.changelog_config();

        assert_eq!(changelog_config.changelog(), ChangelogLocation::Root);
        assert_eq!(
            changelog_config.comparison_links(),
            ComparisonLinksSetting::Disabled
        );

        Ok(())
    }

    #[test]
    fn parse_git_config_defaults() -> anyhow::Result<()> {
        let toml = r#"
[workspace]
members = ["crates/*"]
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_cargo_root_config(dir.path(), CargoRootConfigType::Workspace)?;
        let git_config = config.git_config();

        assert!(git_config.commit());
        assert!(git_config.tags());
        assert!(!git_config.keep_changesets());
        assert_eq!(git_config.tag_format(), TagFormat::VersionOnly);
        assert_eq!(git_config.commit_title_template(), "{new-version}");
        assert!(git_config.changes_in_body());

        Ok(())
    }

    #[test]
    fn parse_git_config_all_options() -> anyhow::Result<()> {
        let toml = r#"
[workspace]
members = ["crates/*"]

[workspace.metadata.changeset]
commit = false
tags = false
keep-changesets = true
tag-format = "crate-prefixed"
commit-title-template = "chore(release): {new-version}"
changes-in-body = false
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_cargo_root_config(dir.path(), CargoRootConfigType::Workspace)?;
        let git_config = config.git_config();

        assert!(!git_config.commit());
        assert!(!git_config.tags());
        assert!(git_config.keep_changesets());
        assert_eq!(git_config.tag_format(), TagFormat::CratePrefixed);
        assert_eq!(
            git_config.commit_title_template(),
            "chore(release): {new-version}"
        );
        assert!(!git_config.changes_in_body());

        Ok(())
    }

    #[test]
    fn parse_git_config_version_only_format() -> anyhow::Result<()> {
        let toml = r#"
[workspace]
members = ["crates/*"]

[workspace.metadata.changeset]
tag-format = "version-only"
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_cargo_root_config(dir.path(), CargoRootConfigType::Workspace)?;
        let git_config = config.git_config();

        assert_eq!(git_config.tag_format(), TagFormat::VersionOnly);

        Ok(())
    }

    #[test]
    fn parse_single_package_git_config() -> anyhow::Result<()> {
        let toml = r#"
[package]
name = "my-crate"
version = "0.1.0"

[package.metadata.changeset]
commit = false
tags = true
keep-changesets = true
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_cargo_root_config(dir.path(), CargoRootConfigType::Package)?;
        let git_config = config.git_config();

        assert!(!git_config.commit());
        assert!(git_config.tags());
        assert!(git_config.keep_changesets());

        Ok(())
    }

    #[test]
    fn parse_zero_version_behavior_default() -> anyhow::Result<()> {
        let toml = r#"
[workspace]
members = ["crates/*"]
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_cargo_root_config(dir.path(), CargoRootConfigType::Workspace)?;

        assert_eq!(
            config.zero_version_behavior(),
            ZeroVersionBehavior::EffectiveMinor
        );

        Ok(())
    }

    #[test]
    fn parse_zero_version_behavior_effective_minor() -> anyhow::Result<()> {
        let toml = r#"
[workspace]
members = ["crates/*"]

[workspace.metadata.changeset]
zero-version-behavior = "effective-minor"
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_cargo_root_config(dir.path(), CargoRootConfigType::Workspace)?;

        assert_eq!(
            config.zero_version_behavior(),
            ZeroVersionBehavior::EffectiveMinor
        );

        Ok(())
    }

    #[test]
    fn parse_zero_version_behavior_auto_promote() -> anyhow::Result<()> {
        let toml = r#"
[workspace]
members = ["crates/*"]

[workspace.metadata.changeset]
zero-version-behavior = "auto-promote-on-major"
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_cargo_root_config(dir.path(), CargoRootConfigType::Workspace)?;

        assert_eq!(
            config.zero_version_behavior(),
            ZeroVersionBehavior::AutoPromoteOnMajor
        );

        Ok(())
    }

    #[test]
    fn parse_single_package_zero_version_behavior() -> anyhow::Result<()> {
        let toml = r#"
[package]
name = "my-crate"
version = "0.1.0"

[package.metadata.changeset]
zero-version-behavior = "auto-promote-on-major"
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_cargo_root_config(dir.path(), CargoRootConfigType::Package)?;

        assert_eq!(
            config.zero_version_behavior(),
            ZeroVersionBehavior::AutoPromoteOnMajor
        );

        Ok(())
    }

    #[test]
    fn parse_dependency_bump_changelog_template_from_workspace_metadata() -> anyhow::Result<()> {
        let toml = r#"
[workspace]
members = ["crates/*"]

[workspace.metadata.changeset]
dependency-bump-changelog-template = "Upgraded `{dependency}` to {version}"
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_cargo_root_config(dir.path(), CargoRootConfigType::Workspace)?;

        assert_eq!(
            config.dependency_bump_changelog_template(),
            "Upgraded `{dependency}` to {version}"
        );

        Ok(())
    }

    #[test]
    fn parse_dependency_bump_changelog_template_from_package_metadata() -> anyhow::Result<()> {
        let toml = r#"
[package]
name = "my-crate"
version = "0.1.0"

[package.metadata.changeset]
dependency-bump-changelog-template = "Bumped `{dependency}` to {version}"
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_cargo_root_config(dir.path(), CargoRootConfigType::Package)?;

        assert_eq!(
            config.dependency_bump_changelog_template(),
            "Bumped `{dependency}` to {version}"
        );

        Ok(())
    }

    #[test]
    fn dependency_bump_changelog_template_defaults_when_not_specified() -> anyhow::Result<()> {
        let toml = r#"
[workspace]
members = ["crates/*"]
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_cargo_root_config(dir.path(), CargoRootConfigType::Workspace)?;

        assert_eq!(
            config.dependency_bump_changelog_template(),
            "Updated dependency `{dependency}` to v{version}"
        );

        Ok(())
    }

    #[test]
    fn parse_workspace_root_config_base_branch_defaults_to_main() -> anyhow::Result<()> {
        let toml = r#"
[workspace]
members = ["crates/*"]
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_cargo_root_config(dir.path(), CargoRootConfigType::Workspace)?;

        assert_eq!(config.base_branch(), "main");

        Ok(())
    }

    #[test]
    fn parse_workspace_root_config_with_base_branch() -> anyhow::Result<()> {
        let toml = r#"
[workspace]
members = ["crates/*"]

[workspace.metadata.changeset]
base-branch = "develop"
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_cargo_root_config(dir.path(), CargoRootConfigType::Workspace)?;

        assert_eq!(config.base_branch(), "develop");

        Ok(())
    }

    #[test]
    fn parse_single_package_root_config_with_base_branch() -> anyhow::Result<()> {
        let toml = r#"
[package]
name = "my-crate"
version = "0.1.0"

[package.metadata.changeset]
base-branch = "master"
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_cargo_root_config(dir.path(), CargoRootConfigType::Package)?;

        assert_eq!(config.base_branch(), "master");

        Ok(())
    }

    #[test]
    fn parse_none_bump_behavior_default() -> anyhow::Result<()> {
        let toml = r#"
[workspace]
members = ["crates/*"]
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_cargo_root_config(dir.path(), CargoRootConfigType::Workspace)?;

        assert_eq!(
            config.none_bump_behavior(),
            changeset_core::NoneBumpBehavior::PromoteToPatch
        );

        Ok(())
    }

    #[test]
    fn parse_none_bump_behavior_allow() -> anyhow::Result<()> {
        let toml = r#"
[workspace]
members = ["crates/*"]

[workspace.metadata.changeset]
none-bump-behavior = "allow"
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_cargo_root_config(dir.path(), CargoRootConfigType::Workspace)?;

        assert_eq!(
            config.none_bump_behavior(),
            changeset_core::NoneBumpBehavior::Allow
        );

        Ok(())
    }

    #[test]
    fn parse_none_bump_behavior_disallow() -> anyhow::Result<()> {
        let toml = r#"
[workspace]
members = ["crates/*"]

[workspace.metadata.changeset]
none-bump-behavior = "disallow"
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_cargo_root_config(dir.path(), CargoRootConfigType::Workspace)?;

        assert_eq!(
            config.none_bump_behavior(),
            changeset_core::NoneBumpBehavior::Disallow
        );

        Ok(())
    }

    #[test]
    fn parse_none_bump_promote_message_template_custom() -> anyhow::Result<()> {
        let toml = r#"
[workspace]
members = ["crates/*"]

[workspace.metadata.changeset]
none-bump-promote-message-template = "chore: internal refactor"
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_cargo_root_config(dir.path(), CargoRootConfigType::Workspace)?;

        assert_eq!(
            config.none_bump_promote_message_template(),
            "chore: internal refactor"
        );

        Ok(())
    }

    #[test]
    fn parse_none_bump_promote_message_template_default() -> anyhow::Result<()> {
        let toml = r#"
[workspace]
members = ["crates/*"]
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_cargo_root_config(dir.path(), CargoRootConfigType::Workspace)?;

        assert_eq!(
            config.none_bump_promote_message_template(),
            "Internal architectural changes"
        );

        Ok(())
    }

    #[test]
    fn parse_additional_packages_from_workspace_metadata() -> anyhow::Result<()> {
        let toml = r#"
[workspace]
members = ["crates/*"]

[[workspace.metadata.changeset.additional-packages]]
name = "my-helm-chart"
path = "charts/my-chart"
influence = ["charts/my-chart/**"]

[workspace.metadata.changeset.additional-packages.manifest]
file-path = "charts/my-chart/Chart.yaml"
format = "yaml"
version-field-path = "version"
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_cargo_root_config(dir.path(), CargoRootConfigType::Workspace)?;
        let packages = config.additional_packages();

        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name(), "my-helm-chart");
        assert_eq!(packages[0].path(), Path::new("charts/my-chart"));
        assert_eq!(packages[0].influence(), &["charts/my-chart/**"]);
        assert_eq!(
            packages[0].manifest().file_path(),
            Path::new("charts/my-chart/Chart.yaml")
        );
        assert_eq!(
            packages[0].manifest().format(),
            changeset_core::ManifestFormat::Yaml
        );
        assert_eq!(packages[0].manifest().version_field_path(), "version");

        Ok(())
    }

    #[test]
    fn parse_additional_packages_empty_by_default() -> anyhow::Result<()> {
        let toml = r#"
[workspace]
members = ["crates/*"]
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_cargo_root_config(dir.path(), CargoRootConfigType::Workspace)?;

        assert!(config.additional_packages().is_empty());

        Ok(())
    }

    #[test]
    fn parse_multiple_additional_packages() -> anyhow::Result<()> {
        let toml = r#"
[workspace]
members = ["crates/*"]

[[workspace.metadata.changeset.additional-packages]]
name = "my-helm-chart"
path = "charts/my-chart"
influence = ["charts/my-chart/**"]

[workspace.metadata.changeset.additional-packages.manifest]
file-path = "charts/my-chart/Chart.yaml"
format = "yaml"
version-field-path = "version"

[[workspace.metadata.changeset.additional-packages]]
name = "my-npm-package"
path = "frontend"
influence = ["frontend/**"]

[workspace.metadata.changeset.additional-packages.manifest]
file-path = "frontend/package.json"
format = "json"
version-field-path = "version"
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_cargo_root_config(dir.path(), CargoRootConfigType::Workspace)?;
        let packages = config.additional_packages();

        assert_eq!(packages.len(), 2);
        assert_eq!(packages[0].name(), "my-helm-chart");
        assert_eq!(
            packages[0].manifest().format(),
            changeset_core::ManifestFormat::Yaml
        );
        assert_eq!(packages[1].name(), "my-npm-package");
        assert_eq!(
            packages[1].manifest().format(),
            changeset_core::ManifestFormat::Json
        );

        Ok(())
    }

    #[test]
    fn parse_additional_packages_from_package_metadata() -> anyhow::Result<()> {
        let toml = r#"
[package]
name = "my-crate"
version = "0.1.0"

[[package.metadata.changeset.additional-packages]]
name = "my-npm-package"
path = "frontend"
influence = ["frontend/**"]

[package.metadata.changeset.additional-packages.manifest]
file-path = "frontend/package.json"
format = "json"
version-field-path = "version"
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_cargo_root_config(dir.path(), CargoRootConfigType::Package)?;
        let packages = config.additional_packages();

        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name(), "my-npm-package");
        assert_eq!(packages[0].path(), Path::new("frontend"));
        assert_eq!(packages[0].influence(), &["frontend/**"]);
        assert_eq!(
            packages[0].manifest().file_path(),
            Path::new("frontend/package.json")
        );
        assert_eq!(
            packages[0].manifest().format(),
            changeset_core::ManifestFormat::Json
        );
        assert_eq!(packages[0].manifest().version_field_path(), "version");

        Ok(())
    }

    #[test]
    fn parse_package_config_with_additional_package_dependencies() -> anyhow::Result<()> {
        let toml = r#"
[package]
name = "my-crate"
version = "0.1.0"

[[package.metadata.changeset.additional-package-dependencies]]
dependency-name = "my-helm-chart"

[package.metadata.changeset.additional-package-dependencies.version-tracking-manifest]
file-path = "src/generated/upstream_version.json"
format = "json"
version-field-path = "upstream_version"
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_package_config(dir.path())?;
        let deps = config.additional_package_dependencies();

        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].dependency_name(), "my-helm-chart");
        assert_eq!(
            deps[0].version_tracking_manifest().file_path(),
            Path::new("src/generated/upstream_version.json")
        );
        assert_eq!(
            deps[0].version_tracking_manifest().format(),
            changeset_core::ManifestFormat::Json
        );
        assert_eq!(
            deps[0].version_tracking_manifest().version_field_path(),
            "upstream_version"
        );

        Ok(())
    }

    #[test]
    fn parse_package_config_without_additional_package_dependencies_defaults_to_empty()
    -> anyhow::Result<()> {
        let toml = r#"
[package]
name = "my-crate"
version = "0.1.0"

[package.metadata.changeset]
ignored-files = ["benches/**"]
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_package_config(dir.path())?;

        assert!(config.additional_package_dependencies().is_empty());

        Ok(())
    }

    #[test]
    fn parse_additional_packages_with_dependencies() -> anyhow::Result<()> {
        let toml = r#"
[workspace]
members = ["crates/*"]

[[workspace.metadata.changeset.additional-packages]]
name = "my-helm-chart"
path = "charts/my-chart"
influence = ["charts/my-chart/**"]

[workspace.metadata.changeset.additional-packages.manifest]
file-path = "charts/my-chart/Chart.yaml"
format = "yaml"
version-field-path = "version"

[[workspace.metadata.changeset.additional-packages.dependencies]]
dependency-name = "upstream-service"

[workspace.metadata.changeset.additional-packages.dependencies.version-tracking-manifest]
file-path = "charts/my-chart/upstream_version.json"
format = "json"
version-field-path = "upstream_version"
"#;
        let dir = setup_with_config(toml)?;

        let config = parse_cargo_root_config(dir.path(), CargoRootConfigType::Workspace)?;
        let packages = config.additional_packages();

        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name(), "my-helm-chart");

        let deps = packages[0].dependencies();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].dependency_name(), "upstream-service");
        assert_eq!(
            deps[0].version_tracking_manifest().file_path(),
            Path::new("charts/my-chart/upstream_version.json")
        );
        assert_eq!(
            deps[0].version_tracking_manifest().format(),
            changeset_core::ManifestFormat::Json
        );
        assert_eq!(
            deps[0].version_tracking_manifest().version_field_path(),
            "upstream_version"
        );

        Ok(())
    }
}
