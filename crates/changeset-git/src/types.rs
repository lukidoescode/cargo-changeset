use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    pub path: PathBuf,
    pub status: FileStatus,
    pub old_path: Option<PathBuf>,
}

impl FileChange {
    #[must_use]
    pub fn new(path: PathBuf, status: FileStatus) -> Self {
        Self {
            path,
            status,
            old_path: None,
        }
    }

    #[must_use]
    pub fn with_old_path(mut self, old_path: PathBuf) -> Self {
        self.old_path = Some(old_path);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagInfo {
    pub name: String,
    pub target_sha: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitInfo {
    pub sha: String,
    pub message: String,
}
