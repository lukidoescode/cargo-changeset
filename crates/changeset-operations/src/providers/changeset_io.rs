use std::fs;
use std::path::{Path, PathBuf};

use changeset_core::Changeset;
use changeset_parse::{parse_changeset, serialize_changeset};
use changeset_project::CHANGESETS_SUBDIR;
use semver::Version;

use crate::Result;
use crate::error::OperationError;
use crate::traits::{ChangesetReader, ChangesetWriter};

const MAX_FILENAME_ATTEMPTS: usize = 100;

pub struct FileSystemChangesetIO {
    project_root: PathBuf,
}

impl FileSystemChangesetIO {
    #[must_use]
    pub fn new(project_root: &Path) -> Self {
        Self {
            project_root: project_root.to_path_buf(),
        }
    }
}

impl ChangesetReader for FileSystemChangesetIO {
    fn read_changeset(&self, relative_path: &Path) -> Result<Changeset> {
        let full_path = self.project_root.join(relative_path);
        let content =
            fs::read_to_string(&full_path).map_err(|source| OperationError::ChangesetFileRead {
                path: full_path.clone(),
                source,
            })?;
        parse_changeset(&content).map_err(|source| OperationError::ChangesetParse {
            path: full_path,
            source,
        })
    }

    fn list_changesets(&self, changeset_dir: &Path) -> Result<Vec<PathBuf>> {
        self.list_changesets_filtered(changeset_dir, false)
    }

    fn list_consumed_changesets(&self, changeset_dir: &Path) -> Result<Vec<PathBuf>> {
        self.list_changesets_filtered(changeset_dir, true)
    }
}

impl FileSystemChangesetIO {
    fn resolve_base_path(&self, changeset_dir: &Path) -> PathBuf {
        if changeset_dir.is_absolute() {
            changeset_dir.to_path_buf()
        } else {
            self.project_root.join(changeset_dir)
        }
    }

    fn list_changesets_filtered(
        &self,
        changeset_dir: &Path,
        consumed_only: bool,
    ) -> Result<Vec<PathBuf>> {
        let base_path = self.resolve_base_path(changeset_dir);
        let full_path = base_path.join(CHANGESETS_SUBDIR);

        let entries = match fs::read_dir(&full_path) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => {
                return Err(OperationError::ChangesetList {
                    path: full_path,
                    source,
                });
            }
        };

        let mut changesets = Vec::new();

        for entry in entries {
            let entry = entry.map_err(|source| OperationError::ChangesetList {
                path: full_path.clone(),
                source,
            })?;
            let path = entry.path();

            if path.extension().is_none_or(|ext| ext != "md") {
                continue;
            }

            let relative = path
                .strip_prefix(&self.project_root)
                .map_or_else(|_| path.clone(), Path::to_path_buf);

            let content =
                fs::read_to_string(&path).map_err(|source| OperationError::ChangesetFileRead {
                    path: path.clone(),
                    source,
                })?;

            let changeset =
                parse_changeset(&content).map_err(|source| OperationError::ChangesetParse {
                    path: path.clone(),
                    source,
                })?;

            let is_consumed = changeset.consumed_for_prerelease.is_some();

            if consumed_only == is_consumed {
                changesets.push(relative);
            }
        }

        Ok(changesets)
    }
}

impl FileSystemChangesetIO {
    fn resolve_changeset_path(&self, changeset_dir: &Path, path: &Path) -> Result<PathBuf> {
        if path.is_absolute() {
            Ok(path.to_path_buf())
        } else if path.starts_with(changeset_dir) {
            Ok(self.project_root.join(path))
        } else {
            let full_changeset_dir = self.resolve_base_path(changeset_dir);
            let filename =
                path.file_name()
                    .ok_or_else(|| OperationError::InvalidChangesetPath {
                        path: path.to_path_buf(),
                        reason: "path has no filename component",
                    })?;
            Ok(full_changeset_dir.join(CHANGESETS_SUBDIR).join(filename))
        }
    }
}

fn update_changeset_file<F>(full_path: &Path, updater: F) -> Result<()>
where
    F: FnOnce(&mut Changeset),
{
    let content =
        fs::read_to_string(full_path).map_err(|source| OperationError::ChangesetFileRead {
            path: full_path.to_path_buf(),
            source,
        })?;

    let mut changeset =
        parse_changeset(&content).map_err(|source| OperationError::ChangesetParse {
            path: full_path.to_path_buf(),
            source,
        })?;

    updater(&mut changeset);

    let serialized = serialize_changeset(&changeset)?;
    fs::write(full_path, serialized).map_err(OperationError::ChangesetFileWrite)?;

    Ok(())
}

impl ChangesetWriter for FileSystemChangesetIO {
    fn write_changeset(&self, changeset_dir: &Path, changeset: &Changeset) -> Result<String> {
        let changesets_subdir = changeset_dir.join(CHANGESETS_SUBDIR);
        let filename = generate_unique_filename(&changesets_subdir);
        let file_path = changesets_subdir.join(&filename);

        let content = serialize_changeset(changeset)?;
        fs::write(&file_path, content).map_err(OperationError::ChangesetFileWrite)?;

        Ok(filename)
    }

    fn restore_changeset(&self, path: &Path, changeset: &Changeset) -> Result<()> {
        let full_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.project_root.join(path)
        };

        let content = serialize_changeset(changeset)?;
        fs::write(&full_path, content).map_err(OperationError::ChangesetFileWrite)?;

        Ok(())
    }

    fn filename_exists(&self, changeset_dir: &Path, filename: &str) -> bool {
        changeset_dir
            .join(CHANGESETS_SUBDIR)
            .join(filename)
            .exists()
    }

    fn mark_consumed_for_prerelease(
        &self,
        changeset_dir: &Path,
        paths: &[&Path],
        version: &Version,
    ) -> Result<()> {
        let version_string = version.to_string();
        for path in paths {
            let full_path = self.resolve_changeset_path(changeset_dir, path)?;
            update_changeset_file(&full_path, |changeset| {
                changeset.consumed_for_prerelease = Some(version_string.clone());
            })?;
        }
        Ok(())
    }

    fn clear_consumed_for_prerelease(&self, changeset_dir: &Path, paths: &[&Path]) -> Result<()> {
        for path in paths {
            let full_path = self.resolve_changeset_path(changeset_dir, path)?;
            update_changeset_file(&full_path, |changeset| {
                changeset.consumed_for_prerelease = None;
            })?;
        }
        Ok(())
    }
}

fn generate_unique_filename(changeset_dir: &Path) -> String {
    for _ in 0..MAX_FILENAME_ATTEMPTS {
        if let Some(name) = petname::petname(3, "-") {
            let filename = format!("{name}.md");

            if !changeset_dir.join(&filename).exists() {
                return filename;
            }
        }
    }

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("changeset-{timestamp}.md")
}
