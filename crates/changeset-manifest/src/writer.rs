use std::path::Path;

use semver::Version;
use toml_edit::{Item, Table, value};

use crate::config::{InitConfig, MetadataSection};
use crate::error::ManifestError;
use crate::reader::{read_document, read_version};

const DEPENDENCY_SECTIONS: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

/// # Errors
///
/// Returns an error if the manifest cannot be read, parsed, or written.
pub fn write_version(path: &Path, version: &Version) -> Result<(), ManifestError> {
    let mut doc = read_document(path)?;

    let package = doc
        .get_mut("package")
        .ok_or_else(|| ManifestError::MissingField {
            path: path.to_path_buf(),
            field: "package".to_string(),
        })?;

    let package_table = package
        .as_table_like_mut()
        .ok_or_else(|| ManifestError::MissingField {
            path: path.to_path_buf(),
            field: "package (as table)".to_string(),
        })?;

    package_table.insert("version", value(version.to_string()));

    std::fs::write(path, doc.to_string()).map_err(|source| ManifestError::Write {
        path: path.to_path_buf(),
        source,
    })
}

/// # Errors
///
/// Returns an error if the manifest cannot be read, parsed, or written.
pub fn remove_workspace_version(path: &Path) -> Result<(), ManifestError> {
    let mut doc = read_document(path)?;

    let Some(workspace) = doc.get_mut("workspace") else {
        return Ok(());
    };

    let Some(workspace_table) = workspace.as_table_like_mut() else {
        return Ok(());
    };

    let Some(package) = workspace_table.get_mut("package") else {
        return Ok(());
    };

    let Some(package_table) = package.as_table_like_mut() else {
        return Ok(());
    };

    package_table.remove("version");

    std::fs::write(path, doc.to_string()).map_err(|source| ManifestError::Write {
        path: path.to_path_buf(),
        source,
    })
}

/// Writes or restores the workspace package version in a root manifest.
///
/// # Errors
///
/// Returns an error if the manifest cannot be read, parsed, or written.
pub fn write_workspace_version(path: &Path, version: &Version) -> Result<(), ManifestError> {
    let mut doc = read_document(path)?;

    let workspace = doc
        .get_mut("workspace")
        .ok_or_else(|| ManifestError::MissingField {
            path: path.to_path_buf(),
            field: "workspace".to_string(),
        })?;

    let workspace_table =
        workspace
            .as_table_like_mut()
            .ok_or_else(|| ManifestError::MissingField {
                path: path.to_path_buf(),
                field: "workspace (as table)".to_string(),
            })?;

    let package = workspace_table
        .entry("package")
        .or_insert_with(|| Item::Table(Table::new()));

    let package_table = package
        .as_table_like_mut()
        .ok_or_else(|| ManifestError::MissingField {
            path: path.to_path_buf(),
            field: "workspace.package (as table)".to_string(),
        })?;

    package_table.insert("version", value(version.to_string()));

    std::fs::write(path, doc.to_string()).map_err(|source| ManifestError::Write {
        path: path.to_path_buf(),
        source,
    })
}

/// # Errors
///
/// Returns `ManifestError::VerificationFailed` if the version in the manifest
/// does not match the expected version.
pub fn verify_version(path: &Path, expected: &Version) -> Result<(), ManifestError> {
    let actual = read_version(path)?;

    if actual != *expected {
        return Err(ManifestError::VerificationFailed {
            path: path.to_path_buf(),
            expected: expected.to_string(),
            actual: actual.to_string(),
        });
    }

    Ok(())
}

/// Writes changeset configuration to the metadata section of a Cargo.toml file.
///
/// # Errors
///
/// Returns an error if the manifest cannot be read, parsed, or written.
pub fn write_metadata_section(
    path: &Path,
    section: MetadataSection,
    config: &InitConfig,
) -> Result<(), ManifestError> {
    if config.is_empty() {
        return Ok(());
    }

    let mut doc = read_document(path)?;

    let root_key = match section {
        MetadataSection::Workspace => "workspace",
        MetadataSection::Package => "package",
    };

    let root = doc
        .entry(root_key)
        .or_insert_with(|| Item::Table(Table::new()));

    let root_table = root
        .as_table_mut()
        .ok_or_else(|| ManifestError::InvalidSectionType {
            path: path.to_path_buf(),
            section: root_key.to_string(),
        })?;

    let metadata = root_table
        .entry("metadata")
        .or_insert_with(|| Item::Table(Table::new()));

    let metadata_table =
        metadata
            .as_table_mut()
            .ok_or_else(|| ManifestError::InvalidSectionType {
                path: path.to_path_buf(),
                section: format!("{root_key}.metadata"),
            })?;

    let changeset = metadata_table
        .entry("changeset")
        .or_insert_with(|| Item::Table(Table::new()));

    let changeset_table =
        changeset
            .as_table_mut()
            .ok_or_else(|| ManifestError::InvalidSectionType {
                path: path.to_path_buf(),
                section: format!("{root_key}.metadata.changeset"),
            })?;

    changeset_table.set_implicit(true);

    if let Some(commit) = config.commit {
        changeset_table.insert("commit", value(commit));
    }

    if let Some(tags) = config.tags {
        changeset_table.insert("tags", value(tags));
    }

    if let Some(keep_changesets) = config.keep_changesets {
        changeset_table.insert("keep_changesets", value(keep_changesets));
    }

    if let Some(tag_format) = config.tag_format {
        changeset_table.insert("tag_format", value(tag_format.as_str()));
    }

    if let Some(changelog) = config.changelog {
        changeset_table.insert("changelog", value(changelog.as_str()));
    }

    if let Some(comparison_links) = config.comparison_links {
        changeset_table.insert("comparison_links", value(comparison_links.as_str()));
    }

    if let Some(zero_version_behavior) = config.zero_version_behavior {
        changeset_table.insert(
            "zero_version_behavior",
            value(zero_version_behavior.as_str()),
        );
    }

    std::fs::write(path, doc.to_string()).map_err(|source| ManifestError::Write {
        path: path.to_path_buf(),
        source,
    })
}

/// Updates the version of a dependency in all relevant sections of a Cargo.toml.
///
/// Checks `[workspace.dependencies]`, `[dependencies]`, `[dev-dependencies]`,
/// and `[build-dependencies]`. Only updates table-form entries that have an
/// explicit `version` key and do NOT have `workspace = true`.
///
/// # Errors
///
/// Returns an error if the manifest cannot be read, parsed, or written.
pub fn update_dependency_version(
    path: &Path,
    dependency_name: &str,
    new_version: &Version,
) -> Result<bool, ManifestError> {
    let mut doc = read_document(path)?;
    let mut changed = false;

    if let Some(workspace) = doc.get_mut("workspace") {
        if let Some(deps) = workspace.get_mut("dependencies") {
            if update_dep_entry(deps, dependency_name, new_version) {
                changed = true;
            }
        }
    }

    for section in &DEPENDENCY_SECTIONS {
        if let Some(deps) = doc.get_mut(section) {
            if update_dep_entry(deps, dependency_name, new_version) {
                changed = true;
            }
        }
    }

    if changed {
        std::fs::write(path, doc.to_string()).map_err(|source| ManifestError::Write {
            path: path.to_path_buf(),
            source,
        })?;
    }

    Ok(changed)
}

fn update_dep_entry(deps: &mut Item, dep_name: &str, new_version: &Version) -> bool {
    let Some(entry) = deps.get_mut(dep_name) else {
        return false;
    };

    if let Some(table) = entry.as_table_like_mut() {
        let has_workspace_true = table
            .get("workspace")
            .and_then(toml_edit::Item::as_bool)
            .unwrap_or(false);
        if has_workspace_true {
            return false;
        }

        if table.get("version").is_some() {
            table.insert("version", value(new_version.to_string()));
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_version_updates_package_version() {
        let toml = r#"
[package]
name = "test-crate"
version = "1.0.0"
"#;
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, toml).expect("write test file");

        write_version(&path, &Version::new(2, 0, 0)).expect("write version");

        let result = read_version(&path).expect("read version");
        assert_eq!(result, Version::new(2, 0, 0));
    }

    #[test]
    fn write_version_converts_inherited_to_literal() {
        let toml = r#"
[package]
name = "test-crate"
version.workspace = true
"#;
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, toml).expect("write test file");

        write_version(&path, &Version::new(1, 5, 0)).expect("write version");

        let result = read_version(&path).expect("read version");
        assert_eq!(result, Version::new(1, 5, 0));

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(content.contains(r#"version = "1.5.0""#));
        assert!(!content.contains("version.workspace"));
    }

    #[test]
    fn write_version_preserves_comments() {
        let toml = r#"# Package configuration
[package]
name = "test-crate"
# Version comment
version = "1.0.0"
# After version comment
edition = "2021"
"#;
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, toml).expect("write test file");

        write_version(&path, &Version::new(2, 0, 0)).expect("write version");

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(content.contains("# Package configuration"));
        assert!(content.contains("# After version comment"));
    }

    #[test]
    fn remove_workspace_version_removes_field() {
        let toml = r#"
[workspace]
members = ["crates/*"]

[workspace.package]
version = "1.0.0"
edition = "2021"
"#;
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, toml).expect("write test file");

        remove_workspace_version(&path).expect("remove workspace version");

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(!content.contains(r#"version = "1.0.0""#));
        assert!(content.contains(r#"edition = "2021""#));
    }

    #[test]
    fn remove_workspace_version_preserves_other_fields() {
        let toml = r#"
[workspace]
members = ["crates/*"]

[workspace.package]
version = "1.0.0"
edition = "2021"
license = "MIT"
"#;
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, toml).expect("write test file");

        remove_workspace_version(&path).expect("remove workspace version");

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(content.contains(r#"edition = "2021""#));
        assert!(content.contains(r#"license = "MIT""#));
        assert!(content.contains(r#"members = ["crates/*"]"#));
    }

    #[test]
    fn verify_version_succeeds_when_matching() {
        let toml = r#"
[package]
name = "test-crate"
version = "1.2.3"
"#;
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, toml).expect("write test file");

        verify_version(&path, &Version::new(1, 2, 3)).expect("verify version");
    }

    #[test]
    fn verify_version_fails_when_mismatched() {
        let toml = r#"
[package]
name = "test-crate"
version = "1.0.0"
"#;
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, toml).expect("write test file");

        let result = verify_version(&path, &Version::new(2, 0, 0));
        assert!(matches!(
            result,
            Err(ManifestError::VerificationFailed { .. })
        ));
    }

    #[test]
    fn write_metadata_creates_workspace_section() {
        let toml = r#"
[workspace]
members = ["crates/*"]
"#;
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, toml).expect("write test file");

        let config = InitConfig {
            commit: Some(true),
            ..Default::default()
        };

        write_metadata_section(&path, MetadataSection::Workspace, &config).expect("write metadata");

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(content.contains("[workspace.metadata.changeset]"));
        assert!(content.contains("commit = true"));
    }

    #[test]
    fn write_metadata_creates_package_section() {
        let toml = r#"
[package]
name = "test-crate"
version = "1.0.0"
"#;
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, toml).expect("write test file");

        let config = InitConfig {
            tags: Some(true),
            ..Default::default()
        };

        write_metadata_section(&path, MetadataSection::Package, &config).expect("write metadata");

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(content.contains("[package.metadata.changeset]"));
        assert!(content.contains("tags = true"));
    }

    #[test]
    fn write_metadata_preserves_existing_content() {
        let toml = r#"# Workspace configuration
[workspace]
# Members list
members = ["crates/*"]

[workspace.package]
edition = "2021"
"#;
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, toml).expect("write test file");

        let config = InitConfig {
            commit: Some(true),
            ..Default::default()
        };

        write_metadata_section(&path, MetadataSection::Workspace, &config).expect("write metadata");

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(content.contains("# Workspace configuration"));
        assert!(content.contains("# Members list"));
        assert!(content.contains(r#"members = ["crates/*"]"#));
        assert!(content.contains(r#"edition = "2021""#));
    }

    #[test]
    fn write_metadata_updates_existing_section() {
        let toml = r#"
[workspace]
members = ["crates/*"]

[workspace.metadata.changeset]
commit = false
tags = false
"#;
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, toml).expect("write test file");

        let config = InitConfig {
            commit: Some(true),
            tags: Some(true),
            ..Default::default()
        };

        write_metadata_section(&path, MetadataSection::Workspace, &config).expect("write metadata");

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(content.contains("commit = true"));
        assert!(content.contains("tags = true"));
        assert!(!content.contains("commit = false"));
        assert!(!content.contains("tags = false"));
    }

    #[test]
    fn write_metadata_creates_nested_hierarchy() {
        let toml = r#"
[workspace]
members = ["crates/*"]
"#;
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, toml).expect("write test file");

        let config = InitConfig {
            commit: Some(true),
            tags: Some(true),
            ..Default::default()
        };

        write_metadata_section(&path, MetadataSection::Workspace, &config).expect("write metadata");

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(content.contains("[workspace.metadata.changeset]"));
        assert!(content.contains("commit = true"));
        assert!(content.contains("tags = true"));
    }

    #[test]
    fn write_metadata_merges_with_existing_metadata() {
        let toml = r#"
[workspace]
members = ["crates/*"]

[workspace.metadata.other]
key = "value"
"#;
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, toml).expect("write test file");

        let config = InitConfig {
            commit: Some(true),
            ..Default::default()
        };

        write_metadata_section(&path, MetadataSection::Workspace, &config).expect("write metadata");

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(content.contains("[workspace.metadata.other]"));
        assert!(content.contains(r#"key = "value""#));
        assert!(content.contains("[workspace.metadata.changeset]"));
        assert!(content.contains("commit = true"));
    }

    #[test]
    fn write_metadata_handles_all_config_options() {
        use crate::config::{ChangelogLocation, ComparisonLinks, TagFormat, ZeroVersionBehavior};

        let toml = r#"
[workspace]
members = ["crates/*"]
"#;
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, toml).expect("write test file");

        let config = InitConfig {
            commit: Some(true),
            tags: Some(true),
            keep_changesets: Some(false),
            tag_format: Some(TagFormat::CratePrefixed),
            changelog: Some(ChangelogLocation::PerPackage),
            comparison_links: Some(ComparisonLinks::Enabled),
            zero_version_behavior: Some(ZeroVersionBehavior::AutoPromoteOnMajor),
        };

        write_metadata_section(&path, MetadataSection::Workspace, &config).expect("write metadata");

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(content.contains("commit = true"));
        assert!(content.contains("tags = true"));
        assert!(content.contains("keep_changesets = false"));
        assert!(content.contains(r#"tag_format = "crate-prefixed""#));
        assert!(content.contains(r#"changelog = "per-package""#));
        assert!(content.contains(r#"comparison_links = "enabled""#));
        assert!(content.contains(r#"zero_version_behavior = "auto-promote-on-major""#));
    }

    #[test]
    fn write_metadata_skips_none_values() {
        let toml = r#"
[workspace]
members = ["crates/*"]
"#;
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, toml).expect("write test file");

        let config = InitConfig {
            commit: Some(true),
            tags: None,
            keep_changesets: None,
            tag_format: None,
            changelog: None,
            comparison_links: None,
            zero_version_behavior: None,
        };

        write_metadata_section(&path, MetadataSection::Workspace, &config).expect("write metadata");

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(content.contains("commit = true"));
        assert!(!content.contains("tags"));
        assert!(!content.contains("keep_changesets"));
        assert!(!content.contains("tag_format"));
        assert!(!content.contains("changelog"));
        assert!(!content.contains("comparison_links"));
        assert!(!content.contains("zero_version_behavior"));
    }

    #[test]
    fn write_metadata_writes_correct_enum_values() {
        use crate::config::{ChangelogLocation, ComparisonLinks, TagFormat, ZeroVersionBehavior};

        let toml = r#"
[workspace]
members = ["crates/*"]
"#;
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, toml).expect("write test file");

        let config = InitConfig {
            tag_format: Some(TagFormat::VersionOnly),
            changelog: Some(ChangelogLocation::Root),
            comparison_links: Some(ComparisonLinks::Auto),
            zero_version_behavior: Some(ZeroVersionBehavior::EffectiveMinor),
            ..Default::default()
        };

        write_metadata_section(&path, MetadataSection::Workspace, &config).expect("write metadata");

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(content.contains(r#"tag_format = "version-only""#));
        assert!(content.contains(r#"changelog = "root""#));
        assert!(content.contains(r#"comparison_links = "auto""#));
        assert!(content.contains(r#"zero_version_behavior = "effective-minor""#));
    }

    #[test]
    fn write_metadata_empty_config_does_not_modify_file() {
        let toml = r#"
[workspace]
members = ["crates/*"]
"#;
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, toml).expect("write test file");

        let config = InitConfig::default();

        write_metadata_section(&path, MetadataSection::Workspace, &config).expect("write metadata");

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(!content.contains("metadata"));
        assert!(!content.contains("changeset"));
    }

    #[test]
    fn update_dep_version_updates_workspace_deps() {
        let toml = r#"
[workspace]
members = ["crates/*"]

[workspace.dependencies]
my-crate = { path = "crates/my-crate", version = "1.0.0" }
"#;
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, toml).expect("write test file");

        let result =
            update_dependency_version(&path, "my-crate", &Version::new(2, 0, 0)).expect("update");
        assert!(result);

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(content.contains(r#"version = "2.0.0""#));
        assert!(!content.contains(r#"version = "1.0.0""#));
    }

    #[test]
    fn update_dep_version_updates_regular_deps() {
        let toml = r#"
[package]
name = "other-crate"
version = "0.1.0"

[dependencies]
my-crate = { path = "../my-crate", version = "1.0.0" }
"#;
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, toml).expect("write test file");

        let result =
            update_dependency_version(&path, "my-crate", &Version::new(2, 0, 0)).expect("update");
        assert!(result);

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(content.contains(r#"version = "2.0.0""#));
    }

    #[test]
    fn update_dep_version_updates_dev_deps() {
        let toml = r#"
[package]
name = "other-crate"
version = "0.1.0"

[dev-dependencies]
my-crate = { path = "../my-crate", version = "1.0.0" }
"#;
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, toml).expect("write test file");

        let result =
            update_dependency_version(&path, "my-crate", &Version::new(2, 0, 0)).expect("update");
        assert!(result);

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(content.contains(r#"version = "2.0.0""#));
    }

    #[test]
    fn update_dep_version_updates_build_deps() {
        let toml = r#"
[package]
name = "other-crate"
version = "0.1.0"

[build-dependencies]
my-crate = { path = "../my-crate", version = "1.0.0" }
"#;
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, toml).expect("write test file");

        let result =
            update_dependency_version(&path, "my-crate", &Version::new(2, 0, 0)).expect("update");
        assert!(result);

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(content.contains(r#"version = "2.0.0""#));
    }

    #[test]
    fn update_dep_version_skips_workspace_true() {
        let toml = r#"
[package]
name = "other-crate"
version = "0.1.0"

[dependencies]
my-crate = { workspace = true }
"#;
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, toml).expect("write test file");

        let result =
            update_dependency_version(&path, "my-crate", &Version::new(2, 0, 0)).expect("update");
        assert!(!result);

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(content.contains("workspace = true"));
        assert!(!content.contains(r#"version = "2.0.0""#));
    }

    #[test]
    fn update_dep_version_skips_no_version_key() {
        let toml = r#"
[package]
name = "other-crate"
version = "0.1.0"

[dependencies]
my-crate = { path = "../my-crate" }
"#;
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, toml).expect("write test file");

        let result =
            update_dependency_version(&path, "my-crate", &Version::new(2, 0, 0)).expect("update");
        assert!(!result);

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(!content.contains(r#"version = "2.0.0""#));
    }

    #[test]
    fn update_dep_version_skips_missing_dep() {
        let toml = r#"
[package]
name = "other-crate"
version = "0.1.0"

[dependencies]
some-other = "1.0.0"
"#;
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, toml).expect("write test file");

        let result =
            update_dependency_version(&path, "my-crate", &Version::new(2, 0, 0)).expect("update");
        assert!(!result);
    }

    #[test]
    fn update_dep_version_preserves_formatting() {
        let toml = r#"# Root manifest
[workspace]
members = ["crates/*"]

# Workspace deps
[workspace.dependencies]
my-crate = { path = "crates/my-crate", version = "1.0.0" }
"#;
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, toml).expect("write test file");

        update_dependency_version(&path, "my-crate", &Version::new(2, 0, 0)).expect("update");

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(content.contains("# Root manifest"));
        assert!(content.contains("# Workspace deps"));
    }

    #[test]
    fn update_dep_version_skips_simple_string() {
        let toml = r#"
[package]
name = "other-crate"
version = "0.1.0"

[dependencies]
my-crate = "1.0.0"
"#;
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, toml).expect("write test file");

        let result =
            update_dependency_version(&path, "my-crate", &Version::new(2, 0, 0)).expect("update");
        assert!(!result);

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(content.contains(r#"my-crate = "1.0.0""#));
    }

    #[test]
    fn update_dep_version_updates_multiple_sections() {
        let toml = r#"
[package]
name = "other-crate"
version = "0.1.0"

[dependencies]
my-crate = { path = "../my-crate", version = "1.0.0" }

[dev-dependencies]
my-crate = { path = "../my-crate", version = "1.0.0" }
"#;
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, toml).expect("write test file");

        let result =
            update_dependency_version(&path, "my-crate", &Version::new(2, 0, 0)).expect("update");
        assert!(result);

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(!content.contains(r#"version = "1.0.0""#));
        assert_eq!(content.matches(r#"version = "2.0.0""#).count(), 2);
    }

    #[test]
    fn update_dep_version_returns_true_on_change() {
        let toml = r#"
[workspace.dependencies]
my-crate = { path = "crates/my-crate", version = "1.0.0" }
"#;
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, toml).expect("write test file");

        let changed =
            update_dependency_version(&path, "my-crate", &Version::new(2, 0, 0)).expect("update");
        assert!(changed);

        let not_changed = update_dependency_version(&path, "nonexistent", &Version::new(2, 0, 0))
            .expect("update");
        assert!(!not_changed);
    }
}
