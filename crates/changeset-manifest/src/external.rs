use std::path::Path;

use changeset_core::types::ManifestFormat;
use jsonc_parser::ParseOptions;
use jsonc_parser::cst::{CstInputValue, CstObject, CstRootNode};
use semver::Version;
use toml_edit::DocumentMut;
use yaml_edit::Document;

use crate::error::ManifestError;

/// # Errors
///
/// Returns an error if the manifest cannot be read, updated, or written.
pub fn write_external_version(
    path: &Path,
    format: ManifestFormat,
    version_path: &str,
    version: &Version,
) -> Result<(), ManifestError> {
    let content = std::fs::read_to_string(path).map_err(|source| ManifestError::Read {
        path: path.to_path_buf(),
        source,
    })?;

    let new_content = match format {
        ManifestFormat::Toml => write_toml_version(&content, path, version_path, version)?,
        ManifestFormat::Yaml => write_yaml_version(&content, path, version_path, version)?,
        ManifestFormat::Json => write_json_version(&content, path, version_path, version)?,
    };

    std::fs::write(path, new_content).map_err(|source| ManifestError::Write {
        path: path.to_path_buf(),
        source,
    })
}

/// # Errors
///
/// Returns `ManifestError::VerificationFailed` if the version does not match the expected value.
pub fn verify_external_version(
    path: &Path,
    format: ManifestFormat,
    version_path: &str,
    expected: &Version,
) -> Result<(), ManifestError> {
    let content = std::fs::read_to_string(path).map_err(|source| ManifestError::Read {
        path: path.to_path_buf(),
        source,
    })?;

    let version_str = match format {
        ManifestFormat::Toml => read_toml_version(&content, path, version_path)?,
        ManifestFormat::Yaml => read_yaml_version(&content, path, version_path)?,
        ManifestFormat::Json => read_json_version(&content, path, version_path)?,
    };

    let actual =
        version_str
            .parse::<Version>()
            .map_err(|source| ManifestError::InvalidVersion {
                path: path.to_path_buf(),
                version: version_str,
                source,
            })?;

    if actual != *expected {
        return Err(ManifestError::VerificationFailed {
            path: path.to_path_buf(),
            expected: expected.to_string(),
            actual: actual.to_string(),
        });
    }

    Ok(())
}

fn write_toml_version(
    content: &str,
    path: &Path,
    version_path: &str,
    version: &Version,
) -> Result<String, ManifestError> {
    let mut doc = content
        .parse::<DocumentMut>()
        .map_err(|source| ManifestError::Parse {
            path: path.to_path_buf(),
            source,
        })?;

    let segments: Vec<&str> = version_path.split('.').collect();
    let (leaf_key, parent_segments) =
        segments
            .split_last()
            .ok_or_else(|| ManifestError::VersionPathNotFound {
                path: path.to_path_buf(),
                version_path: version_path.to_string(),
            })?;

    let mut current = doc.as_item_mut();
    for segment in parent_segments {
        current = current
            .get_mut(*segment)
            .ok_or_else(|| ManifestError::VersionPathNotFound {
                path: path.to_path_buf(),
                version_path: version_path.to_string(),
            })?;
    }

    let table = current
        .as_table_like_mut()
        .ok_or_else(|| ManifestError::VersionPathNotFound {
            path: path.to_path_buf(),
            version_path: version_path.to_string(),
        })?;

    if table.get(leaf_key).is_none() {
        return Err(ManifestError::VersionPathNotFound {
            path: path.to_path_buf(),
            version_path: version_path.to_string(),
        });
    }

    table.insert(leaf_key, toml_edit::value(version.to_string()));

    Ok(doc.to_string())
}

fn read_toml_version(
    content: &str,
    path: &Path,
    version_path: &str,
) -> Result<String, ManifestError> {
    let doc = content
        .parse::<DocumentMut>()
        .map_err(|source| ManifestError::Parse {
            path: path.to_path_buf(),
            source,
        })?;

    let mut current = doc.as_item();
    for segment in version_path.split('.') {
        current = current
            .get(segment)
            .ok_or_else(|| ManifestError::VersionPathNotFound {
                path: path.to_path_buf(),
                version_path: version_path.to_string(),
            })?;
    }

    current
        .as_str()
        .map(String::from)
        .ok_or_else(|| ManifestError::VersionNotString {
            path: path.to_path_buf(),
            version_path: version_path.to_string(),
        })
}

fn write_yaml_version(
    content: &str,
    path: &Path,
    version_path: &str,
    version: &Version,
) -> Result<String, ManifestError> {
    let doc = content
        .parse::<Document>()
        .map_err(|source| ManifestError::YamlParse {
            path: path.to_path_buf(),
            source,
        })?;

    let segments: Vec<&str> = version_path.split('.').collect();
    let (leaf_key, parent_segments) =
        segments
            .split_last()
            .ok_or_else(|| ManifestError::VersionPathNotFound {
                path: path.to_path_buf(),
                version_path: version_path.to_string(),
            })?;

    let mapping = doc
        .as_mapping()
        .ok_or_else(|| ManifestError::VersionPathNotFound {
            path: path.to_path_buf(),
            version_path: version_path.to_string(),
        })?;

    let mut current_mapping = mapping;
    for segment in parent_segments {
        current_mapping = current_mapping.get_mapping(*segment).ok_or_else(|| {
            ManifestError::VersionPathNotFound {
                path: path.to_path_buf(),
                version_path: version_path.to_string(),
            }
        })?;
    }

    if !current_mapping.contains_key(*leaf_key) {
        return Err(ManifestError::VersionPathNotFound {
            path: path.to_path_buf(),
            version_path: version_path.to_string(),
        });
    }

    current_mapping.set(*leaf_key, version.to_string().as_str());

    Ok(doc.to_string())
}

fn read_yaml_version(
    content: &str,
    path: &Path,
    version_path: &str,
) -> Result<String, ManifestError> {
    let doc = content
        .parse::<Document>()
        .map_err(|source| ManifestError::YamlParse {
            path: path.to_path_buf(),
            source,
        })?;

    let segments: Vec<&str> = version_path.split('.').collect();
    let (leaf_key, parent_segments) =
        segments
            .split_last()
            .ok_or_else(|| ManifestError::VersionPathNotFound {
                path: path.to_path_buf(),
                version_path: version_path.to_string(),
            })?;

    let mapping = doc
        .as_mapping()
        .ok_or_else(|| ManifestError::VersionPathNotFound {
            path: path.to_path_buf(),
            version_path: version_path.to_string(),
        })?;

    let mut current_mapping = mapping;
    for segment in parent_segments {
        current_mapping = current_mapping.get_mapping(*segment).ok_or_else(|| {
            ManifestError::VersionPathNotFound {
                path: path.to_path_buf(),
                version_path: version_path.to_string(),
            }
        })?;
    }

    let node =
        current_mapping
            .get(*leaf_key)
            .ok_or_else(|| ManifestError::VersionPathNotFound {
                path: path.to_path_buf(),
                version_path: version_path.to_string(),
            })?;

    node.as_scalar()
        .map(yaml_edit::Scalar::as_string)
        .ok_or_else(|| ManifestError::VersionNotString {
            path: path.to_path_buf(),
            version_path: version_path.to_string(),
        })
}

fn navigate_json_to_object(
    root_obj: &CstObject,
    path: &Path,
    version_path: &str,
    parent_segments: &[&str],
) -> Result<CstObject, ManifestError> {
    let mut current = root_obj.clone();
    for segment in parent_segments {
        current =
            current
                .object_value(segment)
                .ok_or_else(|| ManifestError::VersionPathNotFound {
                    path: path.to_path_buf(),
                    version_path: version_path.to_string(),
                })?;
    }
    Ok(current)
}

fn write_json_version(
    content: &str,
    path: &Path,
    version_path: &str,
    version: &Version,
) -> Result<String, ManifestError> {
    let root = CstRootNode::parse(content, &ParseOptions::default()).map_err(|source| {
        ManifestError::JsonParse {
            path: path.to_path_buf(),
            source,
        }
    })?;

    let root_obj = root
        .object_value()
        .ok_or_else(|| ManifestError::VersionPathNotFound {
            path: path.to_path_buf(),
            version_path: version_path.to_string(),
        })?;

    let segments: Vec<&str> = version_path.split('.').collect();
    let (leaf_key, parent_segments) =
        segments
            .split_last()
            .ok_or_else(|| ManifestError::VersionPathNotFound {
                path: path.to_path_buf(),
                version_path: version_path.to_string(),
            })?;

    let target_obj = navigate_json_to_object(&root_obj, path, version_path, parent_segments)?;

    let prop = target_obj
        .get(leaf_key)
        .ok_or_else(|| ManifestError::VersionPathNotFound {
            path: path.to_path_buf(),
            version_path: version_path.to_string(),
        })?;

    prop.set_value(CstInputValue::String(version.to_string()));

    Ok(root.to_string())
}

fn read_json_version(
    content: &str,
    path: &Path,
    version_path: &str,
) -> Result<String, ManifestError> {
    let root = CstRootNode::parse(content, &ParseOptions::default()).map_err(|source| {
        ManifestError::JsonParse {
            path: path.to_path_buf(),
            source,
        }
    })?;

    let root_obj = root
        .object_value()
        .ok_or_else(|| ManifestError::VersionPathNotFound {
            path: path.to_path_buf(),
            version_path: version_path.to_string(),
        })?;

    let segments: Vec<&str> = version_path.split('.').collect();
    let (leaf_key, parent_segments) =
        segments
            .split_last()
            .ok_or_else(|| ManifestError::VersionPathNotFound {
                path: path.to_path_buf(),
                version_path: version_path.to_string(),
            })?;

    let target_obj = navigate_json_to_object(&root_obj, path, version_path, parent_segments)?;

    let prop = target_obj
        .get(leaf_key)
        .ok_or_else(|| ManifestError::VersionPathNotFound {
            path: path.to_path_buf(),
            version_path: version_path.to_string(),
        })?;

    let node = prop
        .value()
        .ok_or_else(|| ManifestError::VersionNotString {
            path: path.to_path_buf(),
            version_path: version_path.to_string(),
        })?;

    let string_lit = node
        .as_string_lit()
        .ok_or_else(|| ManifestError::VersionNotString {
            path: path.to_path_buf(),
            version_path: version_path.to_string(),
        })?;

    string_lit
        .decoded_value()
        .map_err(|source| ManifestError::JsonStringDecode {
            path: path.to_path_buf(),
            source,
        })
}

#[cfg(test)]
mod tests {
    use semver::Version;
    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn writes_toml_version_preserving_comments() {
        let content = "# package info\n[package]\nname = \"foo\"\nversion = \"1.0.0\"\n# after version\nedition = \"2024\"\n";
        let file = NamedTempFile::new().expect("create temp file");
        std::fs::write(file.path(), content).expect("write file");

        write_external_version(
            file.path(),
            ManifestFormat::Toml,
            "package.version",
            &Version::new(2, 0, 0),
        )
        .expect("write version");

        let result = std::fs::read_to_string(file.path()).expect("read file");
        assert!(result.contains("# package info"));
        assert!(result.contains("# after version"));
        assert!(result.contains(r#"version = "2.0.0""#));
    }

    #[test]
    fn writes_yaml_version_preserving_comments() {
        let content = "name: my-chart # chart name\nversion: \"1.0.0\" # current version\n";
        let file = NamedTempFile::new().expect("create temp file");
        std::fs::write(file.path(), content).expect("write file");

        write_external_version(
            file.path(),
            ManifestFormat::Yaml,
            "version",
            &Version::new(2, 0, 0),
        )
        .expect("write version");

        let result = std::fs::read_to_string(file.path()).expect("read file");
        assert!(result.contains("# chart name"));
        assert!(result.contains("2.0.0"));
    }

    #[test]
    fn writes_json_version_preserving_formatting() {
        let content = "{\n  \"version\": \"1.0.0\",\n  \"name\": \"my-pkg\"\n}\n";
        let file = NamedTempFile::new().expect("create temp file");
        std::fs::write(file.path(), content).expect("write file");

        write_external_version(
            file.path(),
            ManifestFormat::Json,
            "version",
            &Version::new(2, 0, 0),
        )
        .expect("write version");

        let result = std::fs::read_to_string(file.path()).expect("read file");
        assert!(result.contains("  \"version\""));
        assert!(result.contains("\"2.0.0\""));
    }

    #[test]
    fn writes_jsonc_version_preserving_comments() {
        let content = "{\n  // version field\n  \"version\": \"1.0.0\"\n}\n";
        let file = NamedTempFile::new().expect("create temp file");
        std::fs::write(file.path(), content).expect("write file");

        write_external_version(
            file.path(),
            ManifestFormat::Json,
            "version",
            &Version::new(2, 0, 0),
        )
        .expect("write version");

        let result = std::fs::read_to_string(file.path()).expect("read file");
        assert!(result.contains("// version field"));
        assert!(result.contains("\"2.0.0\""));
    }

    #[test]
    fn writes_yaml_version_flat_path() {
        let content = "version: \"1.0.0\"\nname: my-chart\n";
        let file = NamedTempFile::new().expect("create temp file");
        std::fs::write(file.path(), content).expect("write file");

        write_external_version(
            file.path(),
            ManifestFormat::Yaml,
            "version",
            &Version::new(3, 1, 4),
        )
        .expect("write version");

        let result = std::fs::read_to_string(file.path()).expect("read file");
        assert!(result.contains("3.1.4"));
    }

    #[test]
    fn writes_toml_version_nested_path() {
        let content = "[package]\nname = \"my-crate\"\nversion = \"1.0.0\"\n";
        let file = NamedTempFile::new().expect("create temp file");
        std::fs::write(file.path(), content).expect("write file");

        write_external_version(
            file.path(),
            ManifestFormat::Toml,
            "package.version",
            &Version::new(1, 2, 3),
        )
        .expect("write version");

        let result = std::fs::read_to_string(file.path()).expect("read file");
        assert!(result.contains(r#"version = "1.2.3""#));
    }

    #[test]
    fn writes_json_version_nested_path() {
        let content = "{\n  \"metadata\": {\n    \"version\": \"1.0.0\"\n  }\n}\n";
        let file = NamedTempFile::new().expect("create temp file");
        std::fs::write(file.path(), content).expect("write file");

        write_external_version(
            file.path(),
            ManifestFormat::Json,
            "metadata.version",
            &Version::new(2, 0, 0),
        )
        .expect("write version");

        let result = std::fs::read_to_string(file.path()).expect("read file");
        assert!(result.contains("\"2.0.0\""));
    }

    #[test]
    fn verifies_matching_version() {
        let content = "{\n  \"version\": \"1.2.3\"\n}\n";
        let file = NamedTempFile::new().expect("create temp file");
        std::fs::write(file.path(), content).expect("write file");

        verify_external_version(
            file.path(),
            ManifestFormat::Json,
            "version",
            &Version::new(1, 2, 3),
        )
        .expect("verify version");
    }

    #[test]
    fn verifies_returns_error_on_mismatch() {
        let content = "{\n  \"version\": \"1.0.0\"\n}\n";
        let file = NamedTempFile::new().expect("create temp file");
        std::fs::write(file.path(), content).expect("write file");

        let result = verify_external_version(
            file.path(),
            ManifestFormat::Json,
            "version",
            &Version::new(2, 0, 0),
        );
        assert!(matches!(
            result,
            Err(ManifestError::VerificationFailed { .. })
        ));
    }

    #[test]
    fn returns_error_for_missing_version_path() {
        let content = "{\n  \"name\": \"my-pkg\"\n}\n";
        let file = NamedTempFile::new().expect("create temp file");
        std::fs::write(file.path(), content).expect("write file");

        let result = write_external_version(
            file.path(),
            ManifestFormat::Json,
            "version",
            &Version::new(1, 0, 0),
        );
        assert!(matches!(
            result,
            Err(ManifestError::VersionPathNotFound { .. })
        ));
    }
}
