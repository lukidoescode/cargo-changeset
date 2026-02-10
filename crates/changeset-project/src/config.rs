use std::collections::HashMap;
use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};

use crate::error::ProjectError;
use crate::manifest::read_manifest;
use crate::project::{CargoProject, ProjectKind};

#[derive(Debug)]
pub struct RootChangesetConfig {
    ignored_files: GlobSet,
    changeset_dir: PathBuf,
}

impl Default for RootChangesetConfig {
    fn default() -> Self {
        Self {
            ignored_files: GlobSet::empty(),
            changeset_dir: PathBuf::from(crate::DEFAULT_CHANGESET_DIR),
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

    Ok(RootChangesetConfig {
        ignored_files,
        changeset_dir: PathBuf::from(changeset_dir),
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

    Ok(RootChangesetConfig {
        ignored_files,
        changeset_dir: PathBuf::from(changeset_dir),
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
}
