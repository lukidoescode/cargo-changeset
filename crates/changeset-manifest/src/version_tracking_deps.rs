use std::path::Path;

use changeset_core::VersionTrackingDependency;
use toml_edit::{ArrayOfTables, Item, Table, value};

use crate::config::MetadataSection;
use crate::error::ManifestError;
use crate::reader::read_document;
use crate::writer::navigate_to_changeset_table;

fn build_dependency_table(dependency: &VersionTrackingDependency) -> Table {
    let mut table = Table::new();
    table.insert(
        "dependency-name",
        value(dependency.dependency_name().as_str()),
    );

    let mut manifest_table = Table::new();
    manifest_table.insert(
        "file-path",
        value(
            dependency
                .version_tracking_manifest()
                .file_path()
                .to_string_lossy()
                .as_ref(),
        ),
    );
    manifest_table.insert(
        "format",
        value(
            dependency
                .version_tracking_manifest()
                .format()
                .to_string()
                .as_str(),
        ),
    );
    manifest_table.insert(
        "version-field-path",
        value(
            dependency
                .version_tracking_manifest()
                .version_field_path()
                .as_str(),
        ),
    );
    table.insert("version-tracking-manifest", Item::Table(manifest_table));

    table
}

/// # Errors
///
/// Returns an error if the manifest file cannot be read or written.
pub fn add_version_tracking_dependency_to_additional_package(
    path: &Path,
    section: MetadataSection,
    package_name: &str,
    dependency: &VersionTrackingDependency,
) -> Result<bool, ManifestError> {
    let mut doc = read_document(path)?;
    let changeset_table = navigate_to_changeset_table(&mut doc, section, path)?;

    let Some(aot_item) = changeset_table.get_mut("additional-packages") else {
        return Ok(false);
    };

    let Item::ArrayOfTables(aot) = aot_item else {
        return Ok(false);
    };

    let Some(pkg_table) = aot.iter_mut().find(|t| {
        t.get("name")
            .and_then(Item::as_str)
            .is_some_and(|n| n == package_name)
    }) else {
        return Ok(false);
    };

    let deps_aot = pkg_table
        .entry("dependencies")
        .or_insert(Item::ArrayOfTables(ArrayOfTables::new()));

    let Item::ArrayOfTables(deps) = deps_aot else {
        return Err(ManifestError::InvalidSectionType {
            path: path.to_path_buf(),
            section: "additional-packages[].dependencies".to_string(),
        });
    };

    deps.push(build_dependency_table(dependency));

    std::fs::write(path, doc.to_string()).map_err(|source| ManifestError::Write {
        path: path.to_path_buf(),
        source,
    })?;

    Ok(true)
}

/// Returns `true` if the dependency was found and removed, `false` if not found.
///
/// # Errors
///
/// Returns an error if the manifest file cannot be read or written.
pub fn remove_version_tracking_dependency_from_additional_package(
    path: &Path,
    section: MetadataSection,
    package_name: &str,
    dependency_name: &str,
) -> Result<bool, ManifestError> {
    let mut doc = read_document(path)?;
    let changeset_table = navigate_to_changeset_table(&mut doc, section, path)?;

    let Some(aot_item) = changeset_table.get_mut("additional-packages") else {
        return Ok(false);
    };

    let Item::ArrayOfTables(aot) = aot_item else {
        return Ok(false);
    };

    let Some(pkg_table) = aot.iter_mut().find(|t| {
        t.get("name")
            .and_then(Item::as_str)
            .is_some_and(|n| n == package_name)
    }) else {
        return Ok(false);
    };

    let Some(deps_item) = pkg_table.get_mut("dependencies") else {
        return Ok(false);
    };

    let Item::ArrayOfTables(deps) = deps_item else {
        return Ok(false);
    };

    let original_len = deps.len();
    let indices_to_remove: Vec<usize> = deps
        .iter()
        .enumerate()
        .filter(|(_, t)| {
            t.get("dependency-name")
                .and_then(Item::as_str)
                .is_some_and(|n| n == dependency_name)
        })
        .map(|(i, _)| i)
        .collect();

    if indices_to_remove.is_empty() {
        return Ok(false);
    }

    for i in indices_to_remove.into_iter().rev() {
        deps.remove(i);
    }

    let removed = deps.len() < original_len;
    if removed {
        std::fs::write(path, doc.to_string()).map_err(|source| ManifestError::Write {
            path: path.to_path_buf(),
            source,
        })?;
    }

    Ok(removed)
}

/// # Errors
///
/// Returns an error if the manifest file cannot be read or written.
pub fn add_version_tracking_dependency_to_crate(
    path: &Path,
    dependency: &VersionTrackingDependency,
) -> Result<(), ManifestError> {
    let mut doc = read_document(path)?;
    let changeset_table = navigate_to_changeset_table(&mut doc, MetadataSection::Package, path)?;

    let deps_aot = changeset_table
        .entry("additional-package-dependencies")
        .or_insert(Item::ArrayOfTables(ArrayOfTables::new()));

    let Item::ArrayOfTables(deps) = deps_aot else {
        return Err(ManifestError::InvalidSectionType {
            path: path.to_path_buf(),
            section: "additional-package-dependencies".to_string(),
        });
    };

    deps.push(build_dependency_table(dependency));

    std::fs::write(path, doc.to_string()).map_err(|source| ManifestError::Write {
        path: path.to_path_buf(),
        source,
    })
}

/// Returns `true` if the dependency was found and removed, `false` if not found.
///
/// # Errors
///
/// Returns an error if the manifest file cannot be read or written.
pub fn remove_version_tracking_dependency_from_crate(
    path: &Path,
    dependency_name: &str,
) -> Result<bool, ManifestError> {
    let mut doc = read_document(path)?;
    let changeset_table = navigate_to_changeset_table(&mut doc, MetadataSection::Package, path)?;

    let Some(deps_item) = changeset_table.get_mut("additional-package-dependencies") else {
        return Ok(false);
    };

    let Item::ArrayOfTables(deps) = deps_item else {
        return Ok(false);
    };

    let original_len = deps.len();
    let indices_to_remove: Vec<usize> = deps
        .iter()
        .enumerate()
        .filter(|(_, t)| {
            t.get("dependency-name")
                .and_then(Item::as_str)
                .is_some_and(|n| n == dependency_name)
        })
        .map(|(i, _)| i)
        .collect();

    if indices_to_remove.is_empty() {
        return Ok(false);
    }

    for i in indices_to_remove.into_iter().rev() {
        deps.remove(i);
    }

    let removed = deps.len() < original_len;
    if removed {
        std::fs::write(path, doc.to_string()).map_err(|source| ManifestError::Write {
            path: path.to_path_buf(),
            source,
        })?;
    }

    Ok(removed)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use changeset_core::{ManifestFormat, VersionTrackingManifest};

    use super::*;

    fn make_dependency(dep_name: &str) -> VersionTrackingDependency {
        VersionTrackingDependency::new(
            dep_name.to_string(),
            VersionTrackingManifest::new(
                PathBuf::from("charts/dep/Chart.yaml"),
                ManifestFormat::Yaml,
                "appVersion".to_string(),
            ),
        )
    }

    fn write_temp_toml(content: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, content).expect("write test file");
        (dir, path)
    }

    fn workspace_toml_with_additional_package() -> String {
        r#"
[workspace]
members = ["crates/*"]

[[workspace.metadata.changeset.additional-packages]]
name = "my-chart"
path = "charts/my-chart"
influence = ["charts/my-chart/**"]

[workspace.metadata.changeset.additional-packages.manifest]
file-path = "charts/my-chart/Chart.yaml"
format = "yaml"
version-field-path = "version"
"#
        .to_string()
    }

    fn package_toml() -> String {
        r#"
[package]
name = "my-crate"
version = "0.1.0"
"#
        .to_string()
    }

    #[test]
    fn add_dependency_to_additional_package_creates_entry() {
        let (_dir, path) = write_temp_toml(&workspace_toml_with_additional_package());
        let dep = make_dependency("my-rust-crate");

        let result = add_version_tracking_dependency_to_additional_package(
            &path,
            MetadataSection::Workspace,
            "my-chart",
            &dep,
        )
        .expect("add should succeed");

        assert!(result);
        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(content.contains("dependency-name"));
        assert!(content.contains("my-rust-crate"));
        assert!(content.contains("appVersion"));
    }

    #[test]
    fn add_dependency_to_missing_package_returns_false() {
        let (_dir, path) = write_temp_toml(&workspace_toml_with_additional_package());
        let dep = make_dependency("my-rust-crate");

        let result = add_version_tracking_dependency_to_additional_package(
            &path,
            MetadataSection::Workspace,
            "nonexistent",
            &dep,
        )
        .expect("should not error");

        assert!(!result);
    }

    #[test]
    fn remove_dependency_from_additional_package() {
        let (_dir, path) = write_temp_toml(&workspace_toml_with_additional_package());
        let dep = make_dependency("my-rust-crate");

        add_version_tracking_dependency_to_additional_package(
            &path,
            MetadataSection::Workspace,
            "my-chart",
            &dep,
        )
        .expect("add should succeed");

        let removed = remove_version_tracking_dependency_from_additional_package(
            &path,
            MetadataSection::Workspace,
            "my-chart",
            "my-rust-crate",
        )
        .expect("remove should succeed");

        assert!(removed);
        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(!content.contains("my-rust-crate"));
    }

    #[test]
    fn remove_nonexistent_dependency_returns_false() {
        let (_dir, path) = write_temp_toml(&workspace_toml_with_additional_package());

        let removed = remove_version_tracking_dependency_from_additional_package(
            &path,
            MetadataSection::Workspace,
            "my-chart",
            "nonexistent",
        )
        .expect("should not error");

        assert!(!removed);
    }

    #[test]
    fn add_dependency_to_crate_creates_entry() {
        let (_dir, path) = write_temp_toml(&package_toml());
        let dep = make_dependency("my-helm-chart");

        add_version_tracking_dependency_to_crate(&path, &dep).expect("add should succeed");

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(content.contains("additional-package-dependencies"));
        assert!(content.contains("my-helm-chart"));
        assert!(content.contains("appVersion"));
    }

    #[test]
    fn remove_dependency_from_crate() {
        let (_dir, path) = write_temp_toml(&package_toml());
        let dep = make_dependency("my-helm-chart");

        add_version_tracking_dependency_to_crate(&path, &dep).expect("add should succeed");

        let removed = remove_version_tracking_dependency_from_crate(&path, "my-helm-chart")
            .expect("remove should succeed");

        assert!(removed);
        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(!content.contains("my-helm-chart"));
    }

    #[test]
    fn remove_nonexistent_dependency_from_crate_returns_false() {
        let (_dir, path) = write_temp_toml(&package_toml());

        let removed = remove_version_tracking_dependency_from_crate(&path, "nonexistent")
            .expect("should not error");

        assert!(!removed);
    }
}
