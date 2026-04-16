use std::path::Path;

use changeset_core::types::ManifestFormat;
use semver::Version;

use crate::error::ProjectError;

trait ValueAccess {
    fn get_field(&self, key: &str) -> Option<&Self>;
    fn as_str_value(&self) -> Option<String>;
}

impl ValueAccess for serde_json::Value {
    fn get_field(&self, key: &str) -> Option<&Self> {
        self.get(key)
    }

    fn as_str_value(&self) -> Option<String> {
        self.as_str().map(String::from)
    }
}

impl ValueAccess for serde_yml::Value {
    fn get_field(&self, key: &str) -> Option<&Self> {
        self.get(key)
    }

    fn as_str_value(&self) -> Option<String> {
        if let Some(s) = self.as_str() {
            return Some(s.to_string());
        }
        if let Some(f) = self.as_f64() {
            let s = format!("{f}");
            let parts = s.split('.').count();
            let normalized = match parts {
                1 => format!("{s}.0.0"),
                2 => format!("{s}.0"),
                _ => s,
            };
            return Some(normalized);
        }
        if let Some(i) = self.as_i64() {
            return Some(format!("{i}.0.0"));
        }
        if let Some(u) = self.as_u64() {
            return Some(format!("{u}.0.0"));
        }
        None
    }
}

impl ValueAccess for toml::Value {
    fn get_field(&self, key: &str) -> Option<&Self> {
        self.get(key)
    }

    fn as_str_value(&self) -> Option<String> {
        self.as_str().map(String::from)
    }
}

fn resolve_dot_path<V: ValueAccess>(
    root: &V,
    path: &Path,
    version_field_path: &str,
) -> Result<String, ProjectError> {
    let mut current = root;
    for segment in version_field_path.split('.') {
        current = current.get_field(segment).ok_or_else(|| {
            ProjectError::ExternalVersionPathNotFound {
                path: path.to_path_buf(),
                version_field_path: version_field_path.to_string(),
            }
        })?;
    }
    current
        .as_str_value()
        .ok_or_else(|| ProjectError::ExternalVersionNotString {
            path: path.to_path_buf(),
            version_field_path: version_field_path.to_string(),
        })
}

pub(crate) fn read_external_version(
    path: &Path,
    format: ManifestFormat,
    version_field_path: &str,
) -> Result<Version, ProjectError> {
    let content = std::fs::read_to_string(path).map_err(|source| ProjectError::ManifestRead {
        path: path.to_path_buf(),
        source,
    })?;

    let version_str = match format {
        ManifestFormat::Json => {
            let value: serde_json::Value =
                serde_json::from_str(&content).map_err(|source| ProjectError::JsonParse {
                    path: path.to_path_buf(),
                    source,
                })?;
            resolve_dot_path(&value, path, version_field_path)?
        }
        ManifestFormat::Yaml => {
            let value: serde_yml::Value =
                serde_yml::from_str(&content).map_err(|source| ProjectError::YamlParse {
                    path: path.to_path_buf(),
                    source,
                })?;
            resolve_dot_path(&value, path, version_field_path)?
        }
        ManifestFormat::Toml => {
            let value: toml::Value =
                toml::from_str(&content).map_err(|source| ProjectError::ManifestParse {
                    path: path.to_path_buf(),
                    source,
                })?;
            resolve_dot_path(&value, path, version_field_path)?
        }
    };

    version_str
        .parse::<Version>()
        .map_err(|source| ProjectError::InvalidVersion {
            path: path.to_path_buf(),
            version: version_str,
            source,
        })
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn reads_json_version_flat_path() -> Result<()> {
        let file = NamedTempFile::new()?;
        std::fs::write(file.path(), r#"{ "version": "1.2.3" }"#)?;
        let v = read_external_version(file.path(), ManifestFormat::Json, "version")?;
        assert_eq!(v, Version::new(1, 2, 3));
        Ok(())
    }

    #[test]
    fn reads_json_version_nested_path() -> Result<()> {
        let file = NamedTempFile::new()?;
        std::fs::write(file.path(), r#"{ "metadata": { "version": "2.0.0" } }"#)?;
        let v = read_external_version(file.path(), ManifestFormat::Json, "metadata.version")?;
        assert_eq!(v, Version::new(2, 0, 0));
        Ok(())
    }

    #[test]
    fn reads_yaml_version_simple() -> Result<()> {
        let file = NamedTempFile::new()?;
        std::fs::write(file.path(), "version: \"1.0.0\"\n")?;
        let v = read_external_version(file.path(), ManifestFormat::Yaml, "version")?;
        assert_eq!(v, Version::new(1, 0, 0));
        Ok(())
    }

    #[test]
    fn reads_yaml_version_nested() -> Result<()> {
        let file = NamedTempFile::new()?;
        std::fs::write(file.path(), "metadata:\n  version: \"3.1.4\"\n")?;
        let v = read_external_version(file.path(), ManifestFormat::Yaml, "metadata.version")?;
        assert_eq!(v, Version::new(3, 1, 4));
        Ok(())
    }

    #[test]
    fn reads_toml_version_nested() -> Result<()> {
        let file = NamedTempFile::new()?;
        std::fs::write(file.path(), "[package]\nversion = \"1.0.0\"\n")?;
        let v = read_external_version(file.path(), ManifestFormat::Toml, "package.version")?;
        assert_eq!(v, Version::new(1, 0, 0));
        Ok(())
    }

    #[test]
    fn returns_error_for_missing_version_field_path() -> Result<()> {
        let file = NamedTempFile::new()?;
        std::fs::write(file.path(), r#"{ "name": "my-pkg" }"#)?;
        let result = read_external_version(file.path(), ManifestFormat::Json, "version");
        assert!(matches!(
            result,
            Err(ProjectError::ExternalVersionPathNotFound { .. })
        ));
        Ok(())
    }

    #[test]
    fn returns_error_for_non_string_version() -> Result<()> {
        let file = NamedTempFile::new()?;
        std::fs::write(file.path(), r#"{ "version": { "major": 1 } }"#)?;
        let result = read_external_version(file.path(), ManifestFormat::Json, "version");
        assert!(matches!(
            result,
            Err(ProjectError::ExternalVersionNotString { .. })
        ));
        Ok(())
    }

    #[test]
    fn returns_error_for_invalid_semver() -> Result<()> {
        let file = NamedTempFile::new()?;
        std::fs::write(file.path(), r#"{ "version": "not-a-version" }"#)?;
        let result = read_external_version(file.path(), ManifestFormat::Json, "version");
        assert!(matches!(result, Err(ProjectError::InvalidVersion { .. })));
        Ok(())
    }

    #[test]
    fn reads_yaml_numeric_version() -> Result<()> {
        let file = NamedTempFile::new()?;
        std::fs::write(file.path(), "version: 1.0\n")?;
        let v = read_external_version(file.path(), ManifestFormat::Yaml, "version")?;
        assert_eq!(v, Version::new(1, 0, 0));
        Ok(())
    }
}
