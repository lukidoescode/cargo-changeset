use std::path::Path;

use semver::Version;
use toml_edit::DocumentMut;

use crate::error::ManifestError;

/// # Errors
///
/// Returns `ManifestError::Read` if the file cannot be read, or
/// `ManifestError::Parse` if the TOML is malformed.
pub fn read_document(path: &Path) -> Result<DocumentMut, ManifestError> {
    let content = std::fs::read_to_string(path).map_err(|source| ManifestError::Read {
        path: path.to_path_buf(),
        source,
    })?;

    content
        .parse::<DocumentMut>()
        .map_err(|source| ManifestError::Parse {
            path: path.to_path_buf(),
            source,
        })
}

/// # Errors
///
/// Returns `ManifestError::MissingField` if required fields are absent, or
/// `ManifestError::InvalidVersion` if the version string is not valid semver.
pub fn read_version(path: &Path) -> Result<Version, ManifestError> {
    let doc = read_document(path)?;

    let package = doc
        .get("package")
        .ok_or_else(|| ManifestError::MissingField {
            path: path.to_path_buf(),
            field: "package".to_string(),
        })?;

    let version_item = package
        .get("version")
        .ok_or_else(|| ManifestError::MissingField {
            path: path.to_path_buf(),
            field: "package.version".to_string(),
        })?;

    let version_str = version_item
        .as_str()
        .ok_or_else(|| ManifestError::MissingField {
            path: path.to_path_buf(),
            field: "package.version (as string)".to_string(),
        })?;

    Version::parse(version_str).map_err(|source| ManifestError::InvalidVersion {
        path: path.to_path_buf(),
        version: version_str.to_string(),
        source,
    })
}

/// # Errors
///
/// Returns an error if the manifest cannot be read or parsed.
pub fn has_inherited_version(path: &Path) -> Result<bool, ManifestError> {
    let doc = read_document(path)?;

    let Some(package) = doc.get("package") else {
        return Ok(false);
    };

    let Some(version) = package.get("version") else {
        return Ok(false);
    };

    if let Some(table) = version.as_inline_table() {
        return Ok(table
            .get("workspace")
            .and_then(toml_edit::Value::as_bool)
            .unwrap_or(false));
    }

    if let Some(table) = version.as_table() {
        return Ok(table
            .get("workspace")
            .and_then(toml_edit::Item::as_bool)
            .unwrap_or(false));
    }

    Ok(false)
}

/// # Errors
///
/// Returns an error if the manifest cannot be read or parsed.
pub fn has_workspace_package_version(path: &Path) -> Result<bool, ManifestError> {
    let doc = read_document(path)?;

    let Some(workspace) = doc.get("workspace") else {
        return Ok(false);
    };

    let Some(package) = workspace.get("package") else {
        return Ok(false);
    };

    Ok(package.get("version").is_some())
}

/// Reads the workspace package version from a root manifest.
///
/// # Errors
///
/// Returns an error if the manifest cannot be read, parsed, or if the
/// workspace.package.version field is missing.
pub fn read_workspace_version(path: &Path) -> Result<Version, ManifestError> {
    let doc = read_document(path)?;

    let workspace = doc
        .get("workspace")
        .ok_or_else(|| ManifestError::MissingField {
            path: path.to_path_buf(),
            field: "workspace".to_string(),
        })?;

    let package = workspace
        .get("package")
        .ok_or_else(|| ManifestError::MissingField {
            path: path.to_path_buf(),
            field: "workspace.package".to_string(),
        })?;

    let version_item = package
        .get("version")
        .ok_or_else(|| ManifestError::MissingField {
            path: path.to_path_buf(),
            field: "workspace.package.version".to_string(),
        })?;

    let version_str = version_item
        .as_str()
        .ok_or_else(|| ManifestError::MissingField {
            path: path.to_path_buf(),
            field: "workspace.package.version (as string)".to_string(),
        })?;

    Version::parse(version_str).map_err(|source| ManifestError::InvalidVersion {
        path: path.to_path_buf(),
        version: version_str.to_string(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_version_extracts_semver() {
        let toml = r#"
[package]
name = "test-crate"
version = "1.2.3"
"#;
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, toml).expect("write test file");

        let version = read_version(&path).expect("read version");
        assert_eq!(version, Version::new(1, 2, 3));
    }

    #[test]
    fn has_inherited_version_detects_workspace_true() {
        let toml = r#"
[package]
name = "test-crate"
version.workspace = true
"#;
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, toml).expect("write test file");

        assert!(has_inherited_version(&path).expect("check inherited"));
    }

    #[test]
    fn has_inherited_version_returns_false_for_literal() {
        let toml = r#"
[package]
name = "test-crate"
version = "1.0.0"
"#;
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, toml).expect("write test file");

        assert!(!has_inherited_version(&path).expect("check inherited"));
    }

    #[test]
    fn has_workspace_package_version_detects_root_version() {
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

        assert!(has_workspace_package_version(&path).expect("check workspace version"));
    }

    #[test]
    fn has_workspace_package_version_returns_false_when_missing() {
        let toml = r#"
[workspace]
members = ["crates/*"]

[workspace.package]
edition = "2021"
"#;
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, toml).expect("write test file");

        assert!(!has_workspace_package_version(&path).expect("check workspace version"));
    }
}
