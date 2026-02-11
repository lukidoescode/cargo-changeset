use std::path::Path;

use semver::Version;
use toml_edit::value;

use crate::error::ManifestError;
use crate::reader::{read_document, read_version};

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
}
