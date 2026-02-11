use std::path::{Path, PathBuf};

use changeset_core::Changeset;
use changeset_parse::parse_changeset;

use super::ChangesetReader;
use crate::error::CliError;

pub(crate) struct FileSystemChangesetReader {
    project_root: PathBuf,
}

impl FileSystemChangesetReader {
    pub fn new(project_root: &Path) -> Self {
        Self {
            project_root: project_root.to_path_buf(),
        }
    }
}

impl ChangesetReader for FileSystemChangesetReader {
    fn read_changeset(&self, relative_path: &Path) -> Result<Changeset, CliError> {
        let full_path = self.project_root.join(relative_path);
        let content =
            std::fs::read_to_string(&full_path).map_err(|source| CliError::ChangesetFileRead {
                path: full_path.clone(),
                source,
            })?;
        parse_changeset(&content).map_err(|source| CliError::ChangesetParse {
            path: full_path,
            source,
        })
    }
}
