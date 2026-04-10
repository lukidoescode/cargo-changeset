use std::path::PathBuf;

use changeset_project::{GraduationState, PrereleaseState};
use indexmap::IndexMap;
use semver::Version;

use super::steps::{GraduationStateUpdate, PrereleaseStateUpdate};
use super::types::{ChangelogFileState, ChangesetFileState, GitOptions, ReleaseClassification};
use super::{ChangelogUpdate, CommitResult, GitOperationResult, TagResult};
use crate::types::PackageVersion;

#[derive(Debug, Clone, Copy, Default)]
pub struct SagaReleaseOptions {
    pub classification: ReleaseClassification,
    pub git_options: GitOptions,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) enum ChangesetConsumedState {
    #[default]
    NotConsumed,
    Consumed,
    Cleared,
}

#[derive(Debug, Clone, Default)]
pub struct ReleaseSagaData {
    pub changeset_dir: PathBuf,
    pub root_manifest_path: PathBuf,
    pub inherited_packages: Vec<String>,

    pub planned_releases: Vec<PackageVersion>,
    pub package_paths: IndexMap<String, PathBuf>,
    pub changelog_updates: Vec<ChangelogUpdate>,

    pub classification: ReleaseClassification,
    pub git_options: GitOptions,

    pub prerelease_state_update: Option<PrereleaseStateUpdate>,
    pub graduation_state_update: Option<GraduationStateUpdate>,

    pub changeset_files: Vec<ChangesetFileState>,

    pub manifest_updates: Vec<ManifestUpdate>,
    pub dependency_updates: Vec<DependencyUpdate>,
    pub workspace_version_removed: bool,
    pub original_workspace_version: Option<Version>,

    pub lockfile_backup: Option<Vec<u8>>,
    pub lockfile_path: Option<PathBuf>,

    pub staged_files: Vec<PathBuf>,
    pub files_were_staged: bool,

    pub commit_result: Option<CommitResult>,

    pub tags_created: Vec<TagResult>,

    pub changesets_deleted: Vec<PathBuf>,
    pub consumed_state: ChangesetConsumedState,
    pub consumed_files_cleared: Vec<ChangesetFileState>,

    pub changelog_backups: Vec<ChangelogFileState>,
    pub changelogs_written: bool,
}

#[derive(Debug, Clone)]
pub(super) struct ManifestUpdate {
    pub(super) manifest_path: PathBuf,
    pub(super) old_version: Version,
    pub(super) new_version: Version,
    pub(super) written: bool,
}

#[derive(Debug, Clone)]
pub(super) struct DependencyUpdate {
    pub(super) manifest_path: PathBuf,
    pub(super) dependency_name: String,
    pub(super) old_version: Version,
    pub(super) new_version: Version,
}

impl ReleaseSagaData {
    pub fn new(
        changeset_dir: PathBuf,
        root_manifest_path: PathBuf,
        planned_releases: Vec<PackageVersion>,
        package_paths: IndexMap<String, PathBuf>,
        changelog_updates: Vec<ChangelogUpdate>,
        changeset_files: Vec<PathBuf>,
    ) -> Self {
        let changeset_file_states = changeset_files
            .into_iter()
            .map(|path| ChangesetFileState {
                path,
                original_consumed_status: None,
                backup: None,
            })
            .collect();

        Self {
            changeset_dir,
            root_manifest_path,
            planned_releases,
            package_paths,
            changelog_updates,
            changeset_files: changeset_file_states,
            ..Default::default()
        }
    }

    pub fn with_options(mut self, options: SagaReleaseOptions) -> Self {
        self.classification = options.classification;
        self.git_options = options.git_options;
        self
    }

    pub fn with_inherited_packages(mut self, inherited_packages: Vec<String>) -> Self {
        self.inherited_packages = inherited_packages;
        self
    }

    pub fn with_prerelease_state(mut self, current_state: Option<&PrereleaseState>) -> Self {
        if let Some(state) = current_state {
            let mut new_state = state.clone();
            for release in &self.planned_releases {
                let was_prerelease = changeset_version::is_prerelease(release.current_version());
                let is_now_stable = !changeset_version::is_prerelease(release.new_version());
                if was_prerelease && is_now_stable {
                    let _ = new_state.remove(release.name());
                }
            }
            self.prerelease_state_update = Some(PrereleaseStateUpdate {
                original: Some(state.clone()),
                new_state,
            });
        }
        self
    }

    pub fn with_graduation_state(mut self, current_state: Option<&GraduationState>) -> Self {
        if let Some(state) = current_state {
            let mut new_state = state.clone();
            for release in &self.planned_releases {
                if release.current_version().major == 0 && release.new_version().major >= 1 {
                    let _ = new_state.remove(release.name());
                }
            }
            self.graduation_state_update = Some(GraduationStateUpdate {
                original: Some(state.clone()),
                new_state,
            });
        }
        self
    }

    pub fn with_changelog_backups(mut self, backups: Vec<ChangelogFileState>) -> Self {
        self.changelogs_written = !backups.is_empty();
        self.changelog_backups = backups;
        self
    }

    pub fn into_git_result(self) -> GitOperationResult {
        GitOperationResult::new(
            self.commit_result,
            self.tags_created,
            self.changesets_deleted,
        )
    }
}
