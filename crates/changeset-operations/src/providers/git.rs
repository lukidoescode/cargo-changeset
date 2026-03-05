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
    /// # Errors
    ///
    /// Fails if the project root path cannot be canonicalized.
    pub fn new(project_root: &Path) -> crate::Result<Self> {
        let canonical = project_root.canonicalize().map_err(|source| {
            crate::OperationError::ProjectRootCanonicalize {
                path: project_root.to_path_buf(),
                source,
            }
        })?;
        Ok(Self {
            project_root: canonical,
            repo: Mutex::new(None),
        })
    }

    fn with_repo<T>(
        &self,
        project_root: &Path,
        f: impl FnOnce(&Repository) -> changeset_git::Result<T>,
    ) -> crate::Result<T> {
        let canonical = project_root.canonicalize().map_err(|source| {
            crate::OperationError::ProjectRootCanonicalize {
                path: project_root.to_path_buf(),
                source,
            }
        })?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OperationError;

    fn create_temp_git_repo() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let repo = git2::Repository::init(dir.path()).expect("init git repo");

        let sig = git2::Signature::now("Test", "test@test.com").expect("create signature");
        let tree_id = repo
            .index()
            .expect("get index")
            .write_tree()
            .expect("write tree");
        let tree = repo.find_tree(tree_id).expect("find tree");
        repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
            .expect("create initial commit");

        let canonical = dir.path().canonicalize().expect("canonicalize temp dir");
        (dir, canonical)
    }

    #[test]
    fn opens_repository_successfully() {
        let (_dir, canonical) = create_temp_git_repo();

        let provider = Git2Provider::new(&canonical).expect("should create provider");

        let result: crate::Result<bool> = provider.with_repo(&canonical, |_repo| Ok(true));

        assert!(result.is_ok());
        assert!(result.expect("should succeed"));
    }

    #[test]
    fn returns_mismatch_for_different_paths() {
        let (_dir1, canonical1) = create_temp_git_repo();
        let (_dir2, canonical2) = create_temp_git_repo();

        let provider = Git2Provider::new(&canonical1).expect("should create provider");

        let result: crate::Result<bool> = provider.with_repo(&canonical2, |_repo| Ok(true));

        assert!(result.is_err());
        assert!(matches!(
            result.expect_err("should fail"),
            OperationError::ProjectRootMismatch { .. }
        ));
    }

    #[test]
    fn returns_canonicalize_error_for_nonexistent_path() {
        let result = Git2Provider::new(Path::new("/nonexistent/path/to/project"));

        assert!(result.is_err());
        let err = result.err().expect("should be an error");
        assert!(matches!(
            err,
            OperationError::ProjectRootCanonicalize { .. }
        ));
    }

    #[test]
    fn reuses_cached_repository() {
        let (_dir, canonical) = create_temp_git_repo();

        let provider = Git2Provider::new(&canonical).expect("should create provider");

        let result1: crate::Result<bool> = provider.with_repo(&canonical, |_repo| Ok(true));
        assert!(result1.is_ok());

        assert!(
            provider.repo.lock().expect("lock").is_some(),
            "repository should be cached after first call"
        );

        let result2: crate::Result<bool> = provider.with_repo(&canonical, |_repo| Ok(true));
        assert!(result2.is_ok());
    }
}
