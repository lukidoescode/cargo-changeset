use std::path::Path;

use changeset_git::{CommitInfo, FileChange, Repository, TagInfo};

use crate::Result;
use crate::traits::GitProvider;

pub struct Git2Provider;

impl Git2Provider {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for Git2Provider {
    fn default() -> Self {
        Self::new()
    }
}

impl GitProvider for Git2Provider {
    fn changed_files(
        &self,
        project_root: &Path,
        base: &str,
        head: &str,
    ) -> Result<Vec<FileChange>> {
        let repo = Repository::open(project_root)?;
        Ok(repo.changed_files(Some(base), head)?)
    }

    fn is_working_tree_clean(&self, project_root: &Path) -> Result<bool> {
        let repo = Repository::open(project_root)?;
        Ok(repo.is_working_tree_clean()?)
    }

    fn current_branch(&self, project_root: &Path) -> Result<String> {
        let repo = Repository::open(project_root)?;
        Ok(repo.current_branch()?)
    }

    fn stage_files(&self, project_root: &Path, paths: &[&Path]) -> Result<()> {
        let repo = Repository::open(project_root)?;
        Ok(repo.stage_files(paths)?)
    }

    fn commit(&self, project_root: &Path, message: &str) -> Result<CommitInfo> {
        let repo = Repository::open(project_root)?;
        Ok(repo.commit(message)?)
    }

    fn create_tag(&self, project_root: &Path, tag_name: &str, message: &str) -> Result<TagInfo> {
        let repo = Repository::open(project_root)?;
        Ok(repo.create_tag(tag_name, message)?)
    }

    fn remote_url(&self, project_root: &Path) -> Result<Option<String>> {
        let repo = Repository::open(project_root)?;
        Ok(repo.remote_url()?)
    }

    fn delete_files(&self, project_root: &Path, paths: &[&Path]) -> Result<()> {
        let repo = Repository::open(project_root)?;
        Ok(repo.delete_files(paths)?)
    }

    fn delete_tag(&self, project_root: &Path, tag_name: &str) -> Result<bool> {
        let repo = Repository::open(project_root)?;
        Ok(repo.delete_tag(tag_name)?)
    }

    fn reset_to_parent(&self, project_root: &Path) -> Result<()> {
        let repo = Repository::open(project_root)?;
        Ok(repo.reset_to_parent()?)
    }
}
