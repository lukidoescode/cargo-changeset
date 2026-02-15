use std::collections::HashMap;
use std::path::{Path, PathBuf};

use changeset_changelog::{ChangelogConfig, ChangelogLocation, ComparisonLinksSetting};
use changeset_core::ZeroVersionBehavior;
use globset::{Glob, GlobSet, GlobSetBuilder};

use crate::error::ProjectError;
use crate::manifest::{ChangesetMetadata, TagFormatValue, read_manifest};
use crate::project::{CargoProject, ProjectKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TagFormat {
    #[default]
    VersionOnly,
    CratePrefixed,
}

#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct GitConfig {
    commit: bool,
    tags: bool,
    keep_changesets: bool,
    tag_format: TagFormat,
    commit_title_template: String,
    changes_in_body: bool,
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            commit: true,
            tags: true,
            keep_changesets: false,
            tag_format: TagFormat::default(),
            commit_title_template: String::from("{new-version}"),
            changes_in_body: true,
        }
    }
}

impl GitConfig {
    #[must_use]
    pub fn commit(&self) -> bool {
        self.commit
    }

    #[must_use]
    pub fn tags(&self) -> bool {
        self.tags
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

#[derive(Debug, Clone)]
pub struct RootChangesetConfig {
    ignored_files: GlobSet,
    changeset_dir: PathBuf,
    changelog_config: ChangelogConfig,
    git_config: GitConfig,
    zero_version_behavior: ZeroVersionBehavior,
}

impl Default for RootChangesetConfig {
    fn default() -> Self {
        Self {
            ignored_files: GlobSet::empty(),
            changeset_dir: PathBuf::from(crate::DEFAULT_CHANGESET_DIR),
            changelog_config: ChangelogConfig::default(),
            git_config: GitConfig::default(),
            zero_version_behavior: ZeroVersionBehavior::default(),
        }
    }
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

    #[cfg(any(test, feature = "testing"))]
    #[must_use]
    pub fn with_git_config(mut self, git_config: GitConfig) -> Self {
        self.git_config = git_config;
        self
    }
}

#[derive(Debug, Default)]
pub struct PackageChangesetConfig {
    ignored_files: GlobSet,
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

fn build_changelog_config(
    changelog: Option<ChangelogLocation>,
    comparison_links: Option<ComparisonLinksSetting>,
    comparison_links_template: Option<String>,
) -> ChangelogConfig {
    ChangelogConfig {
        changelog: changelog.unwrap_or_default(),
        comparison_links: comparison_links.unwrap_or_default(),
        comparison_links_template,
    }
}

fn build_git_config(metadata: Option<&ChangesetMetadata>) -> GitConfig {
    let defaults = GitConfig::default();
    match metadata {
        None => defaults,
        Some(cs) => GitConfig {
            commit: cs.commit.unwrap_or(defaults.commit),
            tags: cs.tags.unwrap_or(defaults.tags),
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

/// Parses root configuration from workspace metadata.
///
/// # Errors
///
/// Returns an error if the manifest cannot be read or parsed, or if glob patterns are invalid.
fn parse_workspace_root_config(project_root: &Path) -> Result<RootChangesetConfig, ProjectError> {
    let manifest_path = project_root.join("Cargo.toml");
    let manifest = read_manifest(&manifest_path)?;

    let changeset_metadata = manifest
        .workspace
        .and_then(|ws| ws.metadata)
        .and_then(|meta| meta.changeset);

    let patterns = changeset_metadata
        .as_ref()
        .map(|cs| cs.ignored_files.clone())
        .unwrap_or_default();

    let changeset_dir = changeset_metadata
        .as_ref()
        .and_then(|cs| cs.changeset_dir.clone())
        .unwrap_or_else(|| crate::DEFAULT_CHANGESET_DIR.to_string());

    let ignored_files = build_glob_set(&patterns)?;

    let changelog_config = build_changelog_config(
        changeset_metadata.as_ref().and_then(|cs| cs.changelog),
        changeset_metadata
            .as_ref()
            .and_then(|cs| cs.comparison_links),
        changeset_metadata
            .as_ref()
            .and_then(|cs| cs.comparison_links_template.clone()),
    );

    let git_config = build_git_config(changeset_metadata.as_ref());

    let zero_version_behavior = changeset_metadata
        .as_ref()
        .and_then(|cs| cs.zero_version_behavior)
        .unwrap_or_default();

    Ok(RootChangesetConfig {
        ignored_files,
        changeset_dir: PathBuf::from(changeset_dir),
        changelog_config,
        git_config,
        zero_version_behavior,
    })
}

/// Parses root configuration from package metadata (for single-package projects).
///
/// # Errors
///
/// Returns an error if the manifest cannot be read or parsed, or if glob patterns are invalid.
fn parse_package_root_config(project_root: &Path) -> Result<RootChangesetConfig, ProjectError> {
    let manifest_path = project_root.join("Cargo.toml");
    let manifest = read_manifest(&manifest_path)?;

    let changeset_metadata = manifest
        .package
        .and_then(|pkg| pkg.metadata)
        .and_then(|meta| meta.changeset);

    let patterns = changeset_metadata
        .as_ref()
        .map(|cs| cs.ignored_files.clone())
        .unwrap_or_default();

    let changeset_dir = changeset_metadata
        .as_ref()
        .and_then(|cs| cs.changeset_dir.clone())
        .unwrap_or_else(|| crate::DEFAULT_CHANGESET_DIR.to_string());

    let ignored_files = build_glob_set(&patterns)?;

    let changelog_config = build_changelog_config(
        changeset_metadata.as_ref().and_then(|cs| cs.changelog),
        changeset_metadata
            .as_ref()
            .and_then(|cs| cs.comparison_links),
        changeset_metadata
            .as_ref()
            .and_then(|cs| cs.comparison_links_template.clone()),
    );

    let git_config = build_git_config(changeset_metadata.as_ref());

    let zero_version_behavior = changeset_metadata
        .as_ref()
        .and_then(|cs| cs.zero_version_behavior)
        .unwrap_or_default();

    Ok(RootChangesetConfig {
        ignored_files,
        changeset_dir: PathBuf::from(changeset_dir),
        changelog_config,
        git_config,
        zero_version_behavior,
    })
}

/// Parses the root changeset configuration based on project kind.
///
/// For single-package projects, reads from `[package.metadata.changeset]`.
/// For workspaces, reads from `[workspace.metadata.changeset]`.
///
/// # Errors
///
/// Returns an error if the manifest cannot be read or parsed, or if glob patterns are invalid.
pub fn parse_root_config(project: &CargoProject) -> Result<RootChangesetConfig, ProjectError> {
    match project.kind {
        ProjectKind::SinglePackage => parse_package_root_config(&project.root),
        ProjectKind::VirtualWorkspace | ProjectKind::WorkspaceWithRoot => {
            parse_workspace_root_config(&project.root)
        }
    }
}

/// # Errors
///
/// Returns an error if the manifest cannot be read or parsed, or if glob patterns are invalid.
pub fn parse_package_config(package_path: &Path) -> Result<PackageChangesetConfig, ProjectError> {
    let manifest_path = package_path.join("Cargo.toml");
    let manifest = read_manifest(&manifest_path)?;

    let patterns = manifest
        .package
        .and_then(|pkg| pkg.metadata)
        .and_then(|meta| meta.changeset)
        .map(|cs| cs.ignored_files)
        .unwrap_or_default();

    let ignored_files = build_glob_set(&patterns)?;

    Ok(PackageChangesetConfig { ignored_files })
}

/// # Errors
///
/// Returns an error if any manifest cannot be read or parsed, or if glob patterns are invalid.
pub fn load_changeset_configs(
    project: &CargoProject,
) -> Result<(RootChangesetConfig, HashMap<String, PackageChangesetConfig>), ProjectError> {
    let root_config = parse_root_config(project)?;

    let mut package_configs = HashMap::new();
    for package in &project.packages {
        let config = parse_package_config(&package.path)?;
        package_configs.insert(package.name.clone(), config);
    }

    Ok((root_config, package_configs))
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

        let config = parse_workspace_root_config(dir.path())?;

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

        let config = parse_workspace_root_config(dir.path())?;

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

        let config = parse_workspace_root_config(dir.path())?;

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

        let config = parse_workspace_root_config(dir.path())?;

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

        let config = parse_package_root_config(dir.path())?;

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

        let result = parse_workspace_root_config(dir.path());

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

        let config = parse_workspace_root_config(dir.path())?;

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

        let config = parse_workspace_root_config(dir.path())?;
        let changelog_config = config.changelog_config();

        assert_eq!(changelog_config.changelog, ChangelogLocation::PerPackage);
        assert_eq!(
            changelog_config.comparison_links,
            ComparisonLinksSetting::Enabled
        );
        assert_eq!(
            changelog_config.comparison_links_template.as_deref(),
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

        let config = parse_workspace_root_config(dir.path())?;
        let changelog_config = config.changelog_config();

        assert_eq!(changelog_config.changelog, ChangelogLocation::Root);
        assert_eq!(
            changelog_config.comparison_links,
            ComparisonLinksSetting::Auto
        );
        assert!(changelog_config.comparison_links_template.is_none());

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

        let config = parse_package_root_config(dir.path())?;
        let changelog_config = config.changelog_config();

        assert_eq!(changelog_config.changelog, ChangelogLocation::Root);
        assert_eq!(
            changelog_config.comparison_links,
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

        let config = parse_workspace_root_config(dir.path())?;
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

        let config = parse_workspace_root_config(dir.path())?;
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

        let config = parse_workspace_root_config(dir.path())?;
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

        let config = parse_package_root_config(dir.path())?;
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

        let config = parse_workspace_root_config(dir.path())?;

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

        let config = parse_workspace_root_config(dir.path())?;

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

        let config = parse_workspace_root_config(dir.path())?;

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

        let config = parse_package_root_config(dir.path())?;

        assert_eq!(
            config.zero_version_behavior(),
            ZeroVersionBehavior::AutoPromoteOnMajor
        );

        Ok(())
    }
}
