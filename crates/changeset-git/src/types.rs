use std::path::PathBuf;

use gset::Getset;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    Typechange,
}

#[derive(Debug, Clone, PartialEq, Eq, Getset)]
pub struct FileChange {
    #[getset(get, vis = "pub")]
    path: PathBuf,
    #[getset(get_copy, vis = "pub")]
    status: FileStatus,
    #[getset(get_as_ref, vis = "pub", ty = "Option<&PathBuf>")]
    old_path: Option<PathBuf>,
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

#[derive(Debug, Clone, PartialEq, Eq, Getset)]
pub struct TagInfo {
    #[getset(get, vis = "pub")]
    name: String,
    #[getset(get, vis = "pub")]
    target_sha: String,
}

impl TagInfo {
    #[must_use]
    pub fn new(name: String, target_sha: String) -> Self {
        Self { name, target_sha }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Getset)]
pub struct CommitInfo {
    #[getset(get, vis = "pub")]
    sha: String,
    #[getset(get, vis = "pub")]
    message: String,
}

impl CommitInfo {
    #[must_use]
    pub fn new(sha: String, message: String) -> Self {
        Self { sha, message }
    }
}
