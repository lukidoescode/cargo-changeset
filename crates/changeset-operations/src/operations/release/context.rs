use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::traits::{
    ChangelogWriter, ChangesetReader, ChangesetWriter, GitProvider, ManifestWriter, ReleaseStateIO,
};

pub struct ReleaseSagaContext<G, M, RW, S, C> {
    project_root: PathBuf,
    git_provider: Arc<G>,
    manifest_writer: Arc<M>,
    changeset_rw: Arc<RW>,
    release_state_io: Arc<S>,
    changelog_writer: Arc<C>,
}

impl<G, M, RW, S, C> Clone for ReleaseSagaContext<G, M, RW, S, C> {
    fn clone(&self) -> Self {
        Self {
            project_root: self.project_root.clone(),
            git_provider: Arc::clone(&self.git_provider),
            manifest_writer: Arc::clone(&self.manifest_writer),
            changeset_rw: Arc::clone(&self.changeset_rw),
            release_state_io: Arc::clone(&self.release_state_io),
            changelog_writer: Arc::clone(&self.changelog_writer),
        }
    }
}

impl<G, M, RW, S, C> ReleaseSagaContext<G, M, RW, S, C>
where
    G: GitProvider,
    M: ManifestWriter,
    RW: ChangesetReader + ChangesetWriter,
    S: ReleaseStateIO,
    C: ChangelogWriter,
{
    pub fn new(
        project_root: PathBuf,
        git_provider: Arc<G>,
        manifest_writer: Arc<M>,
        changeset_rw: Arc<RW>,
        release_state_io: Arc<S>,
        changelog_writer: Arc<C>,
    ) -> Self {
        Self {
            project_root,
            git_provider,
            manifest_writer,
            changeset_rw,
            release_state_io,
            changelog_writer,
        }
    }

    #[must_use]
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    #[must_use]
    pub fn git_provider(&self) -> &G {
        &self.git_provider
    }

    #[must_use]
    pub fn manifest_writer(&self) -> &M {
        &self.manifest_writer
    }

    #[must_use]
    pub fn changeset_rw(&self) -> &RW {
        &self.changeset_rw
    }

    #[must_use]
    pub fn release_state_io(&self) -> &S {
        &self.release_state_io
    }

    #[must_use]
    pub fn changelog_writer(&self) -> &C {
        &self.changelog_writer
    }
}
