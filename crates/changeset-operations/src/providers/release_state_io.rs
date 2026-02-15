use std::fs;
use std::path::Path;

use changeset_project::{GraduationState, PrereleaseState};

use crate::Result;
use crate::error::OperationError;
use crate::traits::ReleaseStateIO;

const PRERELEASE_FILENAME: &str = "pre-release.toml";
const GRADUATION_FILENAME: &str = "graduation.toml";

pub struct FileSystemReleaseStateIO;

impl FileSystemReleaseStateIO {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for FileSystemReleaseStateIO {
    fn default() -> Self {
        Self::new()
    }
}

impl ReleaseStateIO for FileSystemReleaseStateIO {
    fn load_prerelease_state(&self, changeset_dir: &Path) -> Result<Option<PrereleaseState>> {
        let path = changeset_dir.join(PRERELEASE_FILENAME);
        load_toml_file(&path)
    }

    fn save_prerelease_state(&self, changeset_dir: &Path, state: &PrereleaseState) -> Result<()> {
        let path = changeset_dir.join(PRERELEASE_FILENAME);
        save_toml_file(&path, state, state.is_empty())
    }

    fn load_graduation_state(&self, changeset_dir: &Path) -> Result<Option<GraduationState>> {
        let path = changeset_dir.join(GRADUATION_FILENAME);
        load_toml_file(&path)
    }

    fn save_graduation_state(&self, changeset_dir: &Path, state: &GraduationState) -> Result<()> {
        let path = changeset_dir.join(GRADUATION_FILENAME);
        save_toml_file(&path, state, state.is_empty())
    }
}

fn load_toml_file<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(path).map_err(|source| OperationError::ReleaseStateRead {
        path: path.to_path_buf(),
        source,
    })?;

    let state = toml::from_str(&content).map_err(|source| OperationError::ReleaseStateParse {
        path: path.to_path_buf(),
        source,
    })?;

    Ok(Some(state))
}

fn save_toml_file<T: serde::Serialize>(
    path: &Path,
    state: &T,
    delete_if_empty: bool,
) -> Result<()> {
    if delete_if_empty {
        match fs::remove_file(path) {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(source) => {
                return Err(OperationError::ReleaseStateWrite {
                    path: path.to_path_buf(),
                    source,
                });
            }
        }
    }

    let content =
        toml::to_string_pretty(state).map_err(|source| OperationError::ReleaseStateSerialize {
            path: path.to_path_buf(),
            source,
        })?;
    fs::write(path, content).map_err(|source| OperationError::ReleaseStateWrite {
        path: path.to_path_buf(),
        source,
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_dir() -> TempDir {
        tempfile::tempdir().expect("failed to create temp dir")
    }

    mod prerelease_state_io {
        use super::*;

        #[test]
        fn load_nonexistent_returns_none() {
            let dir = setup_test_dir();
            let io = FileSystemReleaseStateIO::new();

            let result = io.load_prerelease_state(dir.path());

            assert!(result.is_ok());
            assert!(result.expect("should succeed").is_none());
        }

        #[test]
        fn save_and_load_roundtrip() {
            let dir = setup_test_dir();
            let io = FileSystemReleaseStateIO::new();
            let mut state = PrereleaseState::new();
            state.insert("crate-a".to_string(), "alpha".to_string());
            state.insert("crate-b".to_string(), "beta".to_string());

            io.save_prerelease_state(dir.path(), &state)
                .expect("save should succeed");
            let loaded = io
                .load_prerelease_state(dir.path())
                .expect("load should succeed");

            assert!(loaded.is_some());
            let loaded = loaded.expect("should have state");
            assert_eq!(loaded.get("crate-a"), Some("alpha"));
            assert_eq!(loaded.get("crate-b"), Some("beta"));
        }

        #[test]
        fn save_empty_state_deletes_file() {
            let dir = setup_test_dir();
            let io = FileSystemReleaseStateIO::new();
            let path = dir.path().join(PRERELEASE_FILENAME);

            let mut state = PrereleaseState::new();
            state.insert("crate-a".to_string(), "alpha".to_string());
            io.save_prerelease_state(dir.path(), &state)
                .expect("save should succeed");
            assert!(path.exists());

            let empty_state = PrereleaseState::new();
            io.save_prerelease_state(dir.path(), &empty_state)
                .expect("save should succeed");
            assert!(!path.exists());
        }

        #[test]
        fn save_empty_state_when_file_doesnt_exist_is_noop() {
            let dir = setup_test_dir();
            let io = FileSystemReleaseStateIO::new();
            let path = dir.path().join(PRERELEASE_FILENAME);
            let empty_state = PrereleaseState::new();

            let result = io.save_prerelease_state(dir.path(), &empty_state);

            assert!(result.is_ok());
            assert!(!path.exists());
        }

        #[test]
        fn load_invalid_toml_returns_parse_error() {
            let dir = setup_test_dir();
            let io = FileSystemReleaseStateIO::new();
            let path = dir.path().join(PRERELEASE_FILENAME);
            fs::write(&path, "not valid { toml content").expect("write should succeed");

            let result = io.load_prerelease_state(dir.path());

            let err = result.expect_err("should fail to parse invalid TOML");
            assert!(
                matches!(err, OperationError::ReleaseStateParse { .. }),
                "expected ReleaseStateParse error, got: {err:?}"
            );
        }
    }

    mod graduation_state_io {
        use super::*;

        #[test]
        fn load_nonexistent_returns_none() {
            let dir = setup_test_dir();
            let io = FileSystemReleaseStateIO::new();

            let result = io.load_graduation_state(dir.path());

            assert!(result.is_ok());
            assert!(result.expect("should succeed").is_none());
        }

        #[test]
        fn save_and_load_roundtrip() {
            let dir = setup_test_dir();
            let io = FileSystemReleaseStateIO::new();
            let mut state = GraduationState::new();
            state.add("crate-a".to_string());
            state.add("crate-b".to_string());

            io.save_graduation_state(dir.path(), &state)
                .expect("save should succeed");
            let loaded = io
                .load_graduation_state(dir.path())
                .expect("load should succeed");

            assert!(loaded.is_some());
            let loaded = loaded.expect("should have state");
            assert!(loaded.contains("crate-a"));
            assert!(loaded.contains("crate-b"));
        }

        #[test]
        fn save_empty_state_deletes_file() {
            let dir = setup_test_dir();
            let io = FileSystemReleaseStateIO::new();
            let path = dir.path().join(GRADUATION_FILENAME);

            let mut state = GraduationState::new();
            state.add("crate-a".to_string());
            io.save_graduation_state(dir.path(), &state)
                .expect("save should succeed");
            assert!(path.exists());

            let empty_state = GraduationState::new();
            io.save_graduation_state(dir.path(), &empty_state)
                .expect("save should succeed");
            assert!(!path.exists());
        }

        #[test]
        fn save_empty_state_when_file_doesnt_exist_is_noop() {
            let dir = setup_test_dir();
            let io = FileSystemReleaseStateIO::new();
            let path = dir.path().join(GRADUATION_FILENAME);
            let empty_state = GraduationState::new();

            let result = io.save_graduation_state(dir.path(), &empty_state);

            assert!(result.is_ok());
            assert!(!path.exists());
        }

        #[test]
        fn load_invalid_toml_returns_parse_error() {
            let dir = setup_test_dir();
            let io = FileSystemReleaseStateIO::new();
            let path = dir.path().join(GRADUATION_FILENAME);
            fs::write(&path, "graduation = not an array").expect("write should succeed");

            let result = io.load_graduation_state(dir.path());

            let err = result.expect_err("should fail to parse invalid TOML");
            assert!(
                matches!(err, OperationError::ReleaseStateParse { .. }),
                "expected ReleaseStateParse error, got: {err:?}"
            );
        }
    }

    mod toml_format_validation {
        use super::*;

        #[test]
        fn prerelease_toml_contains_expected_keys() {
            let dir = setup_test_dir();
            let io = FileSystemReleaseStateIO::new();
            let mut state = PrereleaseState::new();
            state.insert("crate-a".to_string(), "alpha".to_string());
            state.insert("crate-b".to_string(), "beta".to_string());

            io.save_prerelease_state(dir.path(), &state)
                .expect("save should succeed");

            let path = dir.path().join(PRERELEASE_FILENAME);
            let content = fs::read_to_string(&path).expect("read file");

            assert!(
                content.contains("crate-a"),
                "TOML should contain crate-a key"
            );
            assert!(content.contains("alpha"), "TOML should contain alpha value");
            assert!(
                content.contains("crate-b"),
                "TOML should contain crate-b key"
            );
            assert!(content.contains("beta"), "TOML should contain beta value");
        }

        #[test]
        fn graduation_toml_contains_graduation_array() {
            let dir = setup_test_dir();
            let io = FileSystemReleaseStateIO::new();
            let mut state = GraduationState::new();
            state.add("crate-a".to_string());
            state.add("crate-b".to_string());

            io.save_graduation_state(dir.path(), &state)
                .expect("save should succeed");

            let path = dir.path().join(GRADUATION_FILENAME);
            let content = fs::read_to_string(&path).expect("read file");

            assert!(
                content.contains("graduation"),
                "TOML should contain graduation key"
            );
            assert!(
                content.contains("crate-a"),
                "TOML should contain crate-a in graduation array"
            );
            assert!(
                content.contains("crate-b"),
                "TOML should contain crate-b in graduation array"
            );
        }

        #[test]
        fn prerelease_toml_is_valid_toml_syntax() {
            let dir = setup_test_dir();
            let io = FileSystemReleaseStateIO::new();
            let mut state = PrereleaseState::new();
            state.insert("my-special-crate".to_string(), "rc".to_string());

            io.save_prerelease_state(dir.path(), &state)
                .expect("save should succeed");

            let path = dir.path().join(PRERELEASE_FILENAME);
            let content = fs::read_to_string(&path).expect("read file");

            let parsed: std::result::Result<toml::Value, _> = toml::from_str(&content);
            assert!(parsed.is_ok(), "output should be valid TOML: {content}");
        }

        #[test]
        fn graduation_toml_is_valid_toml_syntax() {
            let dir = setup_test_dir();
            let io = FileSystemReleaseStateIO::new();
            let mut state = GraduationState::new();
            state.add("my-special-crate".to_string());

            io.save_graduation_state(dir.path(), &state)
                .expect("save should succeed");

            let path = dir.path().join(GRADUATION_FILENAME);
            let content = fs::read_to_string(&path).expect("read file");

            let parsed: std::result::Result<toml::Value, _> = toml::from_str(&content);
            assert!(parsed.is_ok(), "output should be valid TOML: {content}");
        }

        #[test]
        fn prerelease_state_preserves_crate_names_with_hyphens() {
            let dir = setup_test_dir();
            let io = FileSystemReleaseStateIO::new();
            let mut state = PrereleaseState::new();
            state.insert("my-hyphenated-crate-name".to_string(), "alpha".to_string());

            io.save_prerelease_state(dir.path(), &state)
                .expect("save should succeed");
            let loaded = io
                .load_prerelease_state(dir.path())
                .expect("load should succeed")
                .expect("should have state");

            assert_eq!(
                loaded.get("my-hyphenated-crate-name"),
                Some("alpha"),
                "hyphenated crate name should be preserved"
            );
        }

        #[test]
        fn graduation_state_preserves_crate_names_with_hyphens() {
            let dir = setup_test_dir();
            let io = FileSystemReleaseStateIO::new();
            let mut state = GraduationState::new();
            state.add("my-hyphenated-crate-name".to_string());

            io.save_graduation_state(dir.path(), &state)
                .expect("save should succeed");
            let loaded = io
                .load_graduation_state(dir.path())
                .expect("load should succeed")
                .expect("should have state");

            assert!(
                loaded.contains("my-hyphenated-crate-name"),
                "hyphenated crate name should be preserved"
            );
        }

        #[test]
        fn prerelease_state_preserves_custom_tags() {
            let dir = setup_test_dir();
            let io = FileSystemReleaseStateIO::new();
            let mut state = PrereleaseState::new();
            state.insert("crate-a".to_string(), "nightly".to_string());
            state.insert("crate-b".to_string(), "canary".to_string());
            state.insert("crate-c".to_string(), "dev123".to_string());

            io.save_prerelease_state(dir.path(), &state)
                .expect("save should succeed");
            let loaded = io
                .load_prerelease_state(dir.path())
                .expect("load should succeed")
                .expect("should have state");

            assert_eq!(loaded.get("crate-a"), Some("nightly"));
            assert_eq!(loaded.get("crate-b"), Some("canary"));
            assert_eq!(loaded.get("crate-c"), Some("dev123"));
        }
    }

    mod error_handling {
        use super::*;

        #[test]
        fn save_to_nonexistent_parent_fails() {
            let dir = setup_test_dir();
            let io = FileSystemReleaseStateIO::new();
            let nonexistent_path = dir.path().join("nonexistent").join("subdir");

            let mut state = PrereleaseState::new();
            state.insert("crate-a".to_string(), "alpha".to_string());

            let result = io.save_prerelease_state(&nonexistent_path, &state);

            assert!(result.is_err());
            let err = result.expect_err("save should fail for nonexistent directory");
            assert!(
                matches!(err, OperationError::ReleaseStateWrite { .. }),
                "expected ReleaseStateWrite error, got: {err:?}"
            );
        }

        #[test]
        fn load_from_nonexistent_directory_returns_none() {
            let io = FileSystemReleaseStateIO::new();
            let nonexistent_path = std::path::Path::new("/this/path/does/not/exist");

            let result = io.load_prerelease_state(nonexistent_path);

            assert!(result.is_ok());
            assert!(result.expect("should succeed").is_none());
        }

        #[test]
        fn load_graduation_from_nonexistent_directory_returns_none() {
            let io = FileSystemReleaseStateIO::new();
            let nonexistent_path = std::path::Path::new("/this/path/does/not/exist");

            let result = io.load_graduation_state(nonexistent_path);

            assert!(result.is_ok());
            assert!(result.expect("should succeed").is_none());
        }

        #[test]
        fn load_truncated_toml_returns_parse_error() {
            let dir = setup_test_dir();
            let io = FileSystemReleaseStateIO::new();
            let path = dir.path().join(PRERELEASE_FILENAME);
            fs::write(&path, "crate-a = \"alpha").expect("write should succeed");

            let result = io.load_prerelease_state(dir.path());

            let err = result.expect_err("should fail to parse truncated TOML");
            assert!(matches!(err, OperationError::ReleaseStateParse { .. }));
        }

        #[test]
        fn save_to_nonexistent_parent_returns_write_error() {
            let dir = setup_test_dir();
            let io = FileSystemReleaseStateIO::new();
            let nonexistent_path = dir.path().join("nonexistent").join("subdir");

            let mut state = PrereleaseState::new();
            state.insert("crate-a".to_string(), "alpha".to_string());

            let result = io.save_prerelease_state(&nonexistent_path, &state);

            let err = result.expect_err("should fail to write to nonexistent path");
            assert!(
                matches!(err, OperationError::ReleaseStateWrite { .. }),
                "expected ReleaseStateWrite error, got: {err:?}"
            );
        }
    }

    mod default_implementation {
        use super::*;

        #[test]
        fn default_creates_new_instance() {
            let io1 = FileSystemReleaseStateIO::new();
            let io2 = FileSystemReleaseStateIO;

            let dir = setup_test_dir();
            let result1 = io1.load_prerelease_state(dir.path());
            let result2 = io2.load_prerelease_state(dir.path());

            assert!(result1.is_ok());
            assert!(result2.is_ok());
        }
    }
}
