use std::fs;
use std::path::{Path, PathBuf};

use changeset_core::Changeset;
use changeset_parse::{parse_changeset, serialize_changeset};

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
        let full_path = if changeset_dir.is_absolute() {
            changeset_dir.to_path_buf()
        } else {
            self.project_root.join(changeset_dir)
        };

        if !full_path.exists() {
            return Ok(Vec::new());
        }

        let mut changesets = Vec::new();
        let entries = fs::read_dir(&full_path).map_err(|source| OperationError::ChangesetList {
            path: full_path.clone(),
            source,
        })?;

        for entry in entries {
            let entry = entry.map_err(|source| OperationError::ChangesetList {
                path: full_path.clone(),
                source,
            })?;
            let path = entry.path();

            if path.extension().is_some_and(|ext| ext == "md") {
                let relative = path
                    .strip_prefix(&self.project_root)
                    .map(Path::to_path_buf)
                    .unwrap_or(path);
                changesets.push(relative);
            }
        }

        Ok(changesets)
    }
}

impl ChangesetWriter for FileSystemChangesetIO {
    fn write_changeset(&self, changeset_dir: &Path, changeset: &Changeset) -> Result<String> {
        let filename = generate_unique_filename(changeset_dir);
        let file_path = changeset_dir.join(&filename);

        let content = serialize_changeset(changeset)?;
        fs::write(&file_path, content).map_err(OperationError::ChangesetFileWrite)?;

        Ok(filename)
    }

    fn filename_exists(&self, changeset_dir: &Path, filename: &str) -> bool {
        changeset_dir.join(filename).exists()
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
