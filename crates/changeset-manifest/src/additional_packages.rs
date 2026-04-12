use std::path::{Path, PathBuf};

use changeset_core::{AdditionalPackageDeclaration, ManifestFormat};
use toml_edit::{ArrayOfTables, Item, Table, Value, value};

use crate::config::MetadataSection;
use crate::error::ManifestError;
use crate::reader::read_document;
use crate::writer::navigate_to_changeset_table;

pub struct AdditionalPackageUpdate {
    pub path: Option<PathBuf>,
    pub influence: Option<Vec<String>>,
    pub manifest_file_path: Option<PathBuf>,
    pub manifest_format: Option<ManifestFormat>,
    pub manifest_version_field_path: Option<String>,
}

/// # Errors
///
/// Returns an error if the manifest file cannot be read or written, or if the
/// `additional-packages` key exists but is not an array-of-tables.
pub fn add_additional_package(
    path: &Path,
    section: MetadataSection,
    declaration: &AdditionalPackageDeclaration,
) -> Result<(), ManifestError> {
    let mut doc = read_document(path)?;
    let changeset_table = navigate_to_changeset_table(&mut doc, section, path)?;

    let aot = changeset_table
        .entry("additional-packages")
        .or_insert(Item::ArrayOfTables(ArrayOfTables::new()));

    let Item::ArrayOfTables(aot) = aot else {
        return Err(ManifestError::InvalidSectionType {
            path: path.to_path_buf(),
            section: "additional-packages".to_string(),
        });
    };

    let mut table = Table::new();
    table.insert("name", value(declaration.name().as_str()));
    table.insert("path", value(declaration.path().to_string_lossy().as_ref()));

    let mut influence_arr = toml_edit::Array::new();
    for glob in declaration.influence() {
        influence_arr.push(glob.as_str());
    }
    table.insert("influence", Item::Value(Value::Array(influence_arr)));

    let mut manifest_table = Table::new();
    manifest_table.insert(
        "file-path",
        value(
            declaration
                .manifest()
                .file_path()
                .to_string_lossy()
                .as_ref(),
        ),
    );
    manifest_table.insert(
        "format",
        value(declaration.manifest().format().to_string().as_str()),
    );
    manifest_table.insert(
        "version-field-path",
        value(declaration.manifest().version_field_path().as_str()),
    );
    table.insert("manifest", Item::Table(manifest_table));

    aot.push(table);

    std::fs::write(path, doc.to_string()).map_err(|source| ManifestError::Write {
        path: path.to_path_buf(),
        source,
    })
}

/// Returns `true` if the entry was found and removed, `false` if no entry matched `name`.
///
/// # Errors
///
/// Returns an error if the manifest file cannot be read or written.
pub fn remove_additional_package(
    path: &Path,
    section: MetadataSection,
    name: &str,
) -> Result<bool, ManifestError> {
    let mut doc = read_document(path)?;
    let changeset_table = navigate_to_changeset_table(&mut doc, section, path)?;

    let Some(aot_item) = changeset_table.get_mut("additional-packages") else {
        return Ok(false);
    };

    let Item::ArrayOfTables(aot) = aot_item else {
        return Ok(false);
    };

    let original_len = aot.len();
    let indices_to_remove: Vec<usize> = aot
        .iter()
        .enumerate()
        .filter(|(_, t)| {
            t.get("name")
                .and_then(Item::as_str)
                .is_some_and(|n| n == name)
        })
        .map(|(i, _)| i)
        .collect();

    if indices_to_remove.is_empty() {
        return Ok(false);
    }

    for i in indices_to_remove.into_iter().rev() {
        aot.remove(i);
    }

    let removed = aot.len() < original_len;
    if removed {
        std::fs::write(path, doc.to_string()).map_err(|source| ManifestError::Write {
            path: path.to_path_buf(),
            source,
        })?;
    }

    Ok(removed)
}

/// Returns `true` if the entry was found and updated, `false` if no entry matched `name`.
///
/// # Errors
///
/// Returns an error if the manifest file cannot be read or written, or if the manifest
/// sub-table is not a table type.
pub fn update_additional_package(
    path: &Path,
    section: MetadataSection,
    name: &str,
    updates: &AdditionalPackageUpdate,
) -> Result<bool, ManifestError> {
    let mut doc = read_document(path)?;
    let changeset_table = navigate_to_changeset_table(&mut doc, section, path)?;

    let Some(aot_item) = changeset_table.get_mut("additional-packages") else {
        return Ok(false);
    };

    let Item::ArrayOfTables(aot) = aot_item else {
        return Ok(false);
    };

    let Some(table) = aot.iter_mut().find(|t| {
        t.get("name")
            .and_then(Item::as_str)
            .is_some_and(|n| n == name)
    }) else {
        return Ok(false);
    };

    if let Some(ref new_path) = updates.path {
        table.insert("path", value(new_path.to_string_lossy().as_ref()));
    }

    if let Some(ref new_influence) = updates.influence {
        let mut arr = toml_edit::Array::new();
        for glob in new_influence {
            arr.push(glob.as_str());
        }
        table.insert("influence", Item::Value(Value::Array(arr)));
    }

    if updates.manifest_file_path.is_some()
        || updates.manifest_format.is_some()
        || updates.manifest_version_field_path.is_some()
    {
        let manifest_item = table
            .entry("manifest")
            .or_insert_with(|| Item::Table(Table::new()));

        let manifest_table =
            manifest_item
                .as_table_mut()
                .ok_or_else(|| ManifestError::InvalidSectionType {
                    path: path.to_path_buf(),
                    section: "additional-packages[].manifest".to_string(),
                })?;

        if let Some(ref new_file_path) = updates.manifest_file_path {
            manifest_table.insert("file-path", value(new_file_path.to_string_lossy().as_ref()));
        }

        if let Some(new_format) = updates.manifest_format {
            manifest_table.insert("format", value(new_format.to_string().as_str()));
        }

        if let Some(ref new_version_path) = updates.manifest_version_field_path {
            manifest_table.insert("version-field-path", value(new_version_path.as_str()));
        }
    }

    std::fs::write(path, doc.to_string()).map_err(|source| ManifestError::Write {
        path: path.to_path_buf(),
        source,
    })?;

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use changeset_core::{AdditionalPackageManifest, ManifestFormat};
    use std::path::PathBuf;

    fn make_declaration(name: &str) -> AdditionalPackageDeclaration {
        AdditionalPackageDeclaration::new(
            name.to_string(),
            PathBuf::from(format!("charts/{name}")),
            vec![format!("charts/{name}/**")],
            AdditionalPackageManifest::new(
                PathBuf::from(format!("charts/{name}/Chart.yaml")),
                ManifestFormat::Yaml,
                "version".to_string(),
            ),
        )
    }

    fn write_temp_toml(content: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, content).expect("write test file");
        (dir, path)
    }

    #[test]
    fn add_creates_array_of_tables_entry() {
        let (_dir, path) = write_temp_toml(
            r#"
[workspace]
members = ["crates/*"]
"#,
        );
        let decl = make_declaration("my-chart");

        add_additional_package(&path, MetadataSection::Workspace, &decl)
            .expect("add should succeed");

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(content.contains("[[workspace.metadata.changeset.additional-packages]]"));
        assert!(content.contains(r#"name = "my-chart""#));
    }

    #[test]
    fn add_appends_to_existing_array() {
        let (_dir, path) = write_temp_toml(
            r#"
[workspace]
members = ["crates/*"]
"#,
        );

        add_additional_package(
            &path,
            MetadataSection::Workspace,
            &make_declaration("chart-a"),
        )
        .expect("add first");
        add_additional_package(
            &path,
            MetadataSection::Workspace,
            &make_declaration("chart-b"),
        )
        .expect("add second");

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(content.contains(r#"name = "chart-a""#));
        assert!(content.contains(r#"name = "chart-b""#));
    }

    #[test]
    fn add_preserves_existing_comments() {
        let (_dir, path) = write_temp_toml(
            r#"# Workspace config
[workspace]
# Members
members = ["crates/*"]
"#,
        );
        let decl = make_declaration("my-chart");

        add_additional_package(&path, MetadataSection::Workspace, &decl)
            .expect("add should succeed");

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(content.contains("# Workspace config"));
        assert!(content.contains("# Members"));
    }

    #[test]
    fn add_creates_nested_manifest_table() {
        let (_dir, path) = write_temp_toml(
            r#"
[workspace]
members = ["crates/*"]
"#,
        );
        let decl = AdditionalPackageDeclaration::new(
            "my-chart".to_string(),
            PathBuf::from("charts/my-chart"),
            vec![],
            AdditionalPackageManifest::new(
                PathBuf::from("charts/my-chart/Chart.yaml"),
                ManifestFormat::Yaml,
                "version".to_string(),
            ),
        );

        add_additional_package(&path, MetadataSection::Workspace, &decl)
            .expect("add should succeed");

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(content.contains(r#"file-path = "charts/my-chart/Chart.yaml""#));
        assert!(content.contains(r#"format = "yaml""#));
        assert!(content.contains(r#"version-field-path = "version""#));
    }

    #[test]
    fn add_serializes_influence_as_array() {
        let (_dir, path) = write_temp_toml(
            r#"
[workspace]
members = ["crates/*"]
"#,
        );
        let decl = AdditionalPackageDeclaration::new(
            "my-chart".to_string(),
            PathBuf::from("charts/my-chart"),
            vec!["charts/my-chart/**".to_string(), "helm/**".to_string()],
            AdditionalPackageManifest::new(
                PathBuf::from("charts/my-chart/Chart.yaml"),
                ManifestFormat::Yaml,
                "version".to_string(),
            ),
        );

        add_additional_package(&path, MetadataSection::Workspace, &decl)
            .expect("add should succeed");

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(content.contains("influence"));
        assert!(content.contains(r#""charts/my-chart/**""#));
        assert!(content.contains(r#""helm/**""#));
    }

    #[test]
    fn remove_by_name_returns_true() {
        let (_dir, path) = write_temp_toml(
            r#"
[workspace]
members = ["crates/*"]
"#,
        );
        add_additional_package(
            &path,
            MetadataSection::Workspace,
            &make_declaration("my-chart"),
        )
        .expect("add should succeed");

        let result = remove_additional_package(&path, MetadataSection::Workspace, "my-chart")
            .expect("remove should succeed");

        assert!(result);
        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(!content.contains(r#"name = "my-chart""#));
    }

    #[test]
    fn remove_nonexistent_returns_false() {
        let (_dir, path) = write_temp_toml(
            r#"
[workspace]
members = ["crates/*"]
"#,
        );

        let result = remove_additional_package(&path, MetadataSection::Workspace, "nonexistent")
            .expect("remove should succeed");

        assert!(!result);
    }

    #[test]
    fn remove_preserves_other_entries() {
        let (_dir, path) = write_temp_toml(
            r#"
[workspace]
members = ["crates/*"]
"#,
        );
        add_additional_package(
            &path,
            MetadataSection::Workspace,
            &make_declaration("chart-a"),
        )
        .expect("add first");
        add_additional_package(
            &path,
            MetadataSection::Workspace,
            &make_declaration("chart-b"),
        )
        .expect("add second");

        remove_additional_package(&path, MetadataSection::Workspace, "chart-a")
            .expect("remove should succeed");

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(!content.contains(r#"name = "chart-a""#));
        assert!(content.contains(r#"name = "chart-b""#));
    }

    #[test]
    fn remove_preserves_comments() {
        let (_dir, path) = write_temp_toml(
            r#"# Workspace config
[workspace]
# Members comment
members = ["crates/*"]
"#,
        );
        add_additional_package(
            &path,
            MetadataSection::Workspace,
            &make_declaration("chart-a"),
        )
        .expect("add");

        remove_additional_package(&path, MetadataSection::Workspace, "chart-a").expect("remove");

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(content.contains("# Workspace config"));
        assert!(content.contains("# Members comment"));
    }

    #[test]
    fn update_modifies_path_field() {
        let (_dir, path) = write_temp_toml(
            r#"
[workspace]
members = ["crates/*"]
"#,
        );
        add_additional_package(
            &path,
            MetadataSection::Workspace,
            &make_declaration("my-chart"),
        )
        .expect("add");

        let updates = AdditionalPackageUpdate {
            path: Some(PathBuf::from("new/chart/path")),
            influence: None,
            manifest_file_path: None,
            manifest_format: None,
            manifest_version_field_path: None,
        };
        let result =
            update_additional_package(&path, MetadataSection::Workspace, "my-chart", &updates)
                .expect("update should succeed");

        assert!(result);
        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(content.contains(r#"path = "new/chart/path""#));
    }

    #[test]
    fn update_modifies_influence() {
        let (_dir, path) = write_temp_toml(
            r#"
[workspace]
members = ["crates/*"]
"#,
        );
        add_additional_package(
            &path,
            MetadataSection::Workspace,
            &make_declaration("my-chart"),
        )
        .expect("add");

        let updates = AdditionalPackageUpdate {
            path: None,
            influence: Some(vec!["new/pattern/**".to_string()]),
            manifest_file_path: None,
            manifest_format: None,
            manifest_version_field_path: None,
        };
        update_additional_package(&path, MetadataSection::Workspace, "my-chart", &updates)
            .expect("update should succeed");

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(content.contains(r#""new/pattern/**""#));
    }

    #[test]
    fn update_modifies_manifest_fields() {
        let (_dir, path) = write_temp_toml(
            r#"
[workspace]
members = ["crates/*"]
"#,
        );
        add_additional_package(
            &path,
            MetadataSection::Workspace,
            &make_declaration("my-chart"),
        )
        .expect("add");

        let updates = AdditionalPackageUpdate {
            path: None,
            influence: None,
            manifest_file_path: None,
            manifest_format: Some(ManifestFormat::Json),
            manifest_version_field_path: Some("info.version".to_string()),
        };
        update_additional_package(&path, MetadataSection::Workspace, "my-chart", &updates)
            .expect("update should succeed");

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(content.contains(r#"format = "json""#));
        assert!(content.contains(r#"version-field-path = "info.version""#));
    }

    #[test]
    fn update_nonexistent_returns_false() {
        let (_dir, path) = write_temp_toml(
            r#"
[workspace]
members = ["crates/*"]
"#,
        );

        let updates = AdditionalPackageUpdate {
            path: Some(PathBuf::from("somewhere")),
            influence: None,
            manifest_file_path: None,
            manifest_format: None,
            manifest_version_field_path: None,
        };
        let result =
            update_additional_package(&path, MetadataSection::Workspace, "nonexistent", &updates)
                .expect("update should succeed");

        assert!(!result);
    }

    #[test]
    fn update_preserves_comments() {
        let (_dir, path) = write_temp_toml(
            r#"# My project
[workspace]
# Workspace members
members = ["crates/*"]
"#,
        );
        add_additional_package(
            &path,
            MetadataSection::Workspace,
            &make_declaration("my-chart"),
        )
        .expect("add");

        let updates = AdditionalPackageUpdate {
            path: Some(PathBuf::from("new/path")),
            influence: None,
            manifest_file_path: None,
            manifest_format: None,
            manifest_version_field_path: None,
        };
        update_additional_package(&path, MetadataSection::Workspace, "my-chart", &updates)
            .expect("update");

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(content.contains("# My project"));
        assert!(content.contains("# Workspace members"));
    }

    #[test]
    fn workspace_and_package_sections() {
        let (_dir, path) = write_temp_toml(
            r#"
[package]
name = "my-crate"
version = "0.1.0"
"#,
        );
        let decl = make_declaration("my-chart");

        add_additional_package(&path, MetadataSection::Package, &decl).expect("add should succeed");

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(content.contains("[[package.metadata.changeset.additional-packages]]"));
        assert!(!content.contains("[[workspace.metadata.changeset.additional-packages]]"));
    }
}
