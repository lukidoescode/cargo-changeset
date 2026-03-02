use std::path::{Path, PathBuf};
use std::sync::Mutex;

use changeset_git::{CommitInfo, FileChange, Repository, TagInfo};

use crate::traits::{
    GitCommitProvider, GitDiffProvider, GitStagingProvider, GitStatusProvider, GitTagProvider,
};

pub struct Git2Provider {
    project_root: PathBuf,
    repo: Mutex<Option<Repository>>,
}

impl Git2Provider {
    #[must_use]
    pub fn new(project_root: &Path) -> Self {
        let canonical = project_root
            .canonicalize()
            .unwrap_or_else(|_| project_root.to_path_buf());
        Self {
            project_root: canonical,
            repo: Mutex::new(None),
        }
    }

    fn with_repo<T>(
        &self,
        project_root: &Path,
        f: impl FnOnce(&Repository) -> changeset_git::Result<T>,
    ) -> crate::Result<T> {
        let canonical = project_root
            .canonicalize()
            .unwrap_or_else(|_| project_root.to_path_buf());
        if canonical != self.project_root {
            return Err(crate::OperationError::ProjectRootMismatch {
                expected: self.project_root.clone(),
                actual: canonical,
            });
        }
        let mut guard = self
            .repo
            .lock()
            .expect("git repository mutex should not be poisoned");
        if guard.is_none() {
            *guard = Some(Repository::open(&self.project_root)?);
        }
        let repo = guard
            .as_ref()
            .expect("repository should be initialized after open");
        Ok(f(repo)?)
    }
}

impl GitDiffProvider for Git2Provider {
    fn changed_files(
        &self,
        project_root: &Path,
        base: &str,
        head: &str,
    ) -> crate::Result<Vec<FileChange>> {
        self.with_repo(project_root, |repo| repo.changed_files(Some(base), head))
    }
}

impl GitStatusProvider for Git2Provider {
    fn is_working_tree_clean(&self, project_root: &Path) -> crate::Result<bool> {
        self.with_repo(project_root, Repository::is_working_tree_clean)
    }

    fn current_branch(&self, project_root: &Path) -> crate::Result<String> {
        self.with_repo(project_root, Repository::current_branch)
    }

    fn remote_url(&self, project_root: &Path) -> crate::Result<Option<String>> {
        self.with_repo(project_root, Repository::remote_url)
    }
}

impl GitStagingProvider for Git2Provider {
    fn stage_files(&self, project_root: &Path, paths: &[&Path]) -> crate::Result<()> {
        self.with_repo(project_root, |repo| repo.stage_files(paths))
    }

    fn delete_files(&self, project_root: &Path, paths: &[&Path]) -> crate::Result<()> {
        self.with_repo(project_root, |repo| repo.delete_files(paths))
    }
}

impl GitCommitProvider for Git2Provider {
    fn commit(&self, project_root: &Path, message: &str) -> crate::Result<CommitInfo> {
        self.with_repo(project_root, |repo| repo.commit(message))
    }

    fn reset_to_parent(&self, project_root: &Path) -> crate::Result<()> {
        self.with_repo(project_root, Repository::reset_to_parent)
    }
}

impl GitTagProvider for Git2Provider {
    fn create_tag(
        &self,
        project_root: &Path,
        tag_name: &str,
        message: &str,
    ) -> crate::Result<TagInfo> {
        self.with_repo(project_root, |repo| repo.create_tag(tag_name, message))
    }

    fn delete_tag(&self, project_root: &Path, tag_name: &str) -> crate::Result<bool> {
        self.with_repo(project_root, |repo| repo.delete_tag(tag_name))
    }
}
