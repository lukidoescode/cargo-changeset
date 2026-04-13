use std::path::PathBuf;

use changeset_core::ManifestFormat;
use changeset_project::{GraduationState, PrereleaseState};
use gset::Getset;
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

#[derive(Debug, Clone)]
pub(super) enum ManifestKind {
    Cargo,
    #[allow(dead_code)]
    Additional {
        format: ManifestFormat,
        version_field_path: String,
    },
}

#[derive(Debug, Clone)]
pub(super) struct AdditionalManifestInfo {
    pub(super) manifest_path: PathBuf,
    pub(super) format: ManifestFormat,
    pub(super) version_field_path: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) enum ChangesetConsumedState {
    #[default]
    NotConsumed,
    Consumed,
    Cleared,
}

#[derive(Debug, Clone, Default, Getset)]
pub struct ReleaseSagaData {
    #[getset(get, vis = "pub(super)")]
    changeset_dir: PathBuf,
    #[getset(get, vis = "pub(super)")]
    root_manifest_path: PathBuf,
    pub(super) inherited_packages: Vec<String>,

    #[getset(get, vis = "pub(super)")]
    planned_releases: Vec<PackageVersion>,
    #[getset(get, vis = "pub(super)")]
    package_paths: IndexMap<String, PathBuf>,
    #[getset(get, vis = "pub(super)")]
    additional_package_manifests: IndexMap<String, AdditionalManifestInfo>,
    #[getset(get, vis = "pub(super)")]
    changelog_updates: Vec<ChangelogUpdate>,

    pub(super) classification: ReleaseClassification,
    pub(super) git_options: GitOptions,

    #[getset(get, vis = "pub(super)")]
    prerelease_state_update: Option<PrereleaseStateUpdate>,
    #[getset(get, vis = "pub(super)")]
    graduation_state_update: Option<GraduationStateUpdate>,

    pub(super) changeset_files: Vec<ChangesetFileState>,

    pub(super) manifest_updates: Vec<ManifestUpdate>,
    pub(super) dependency_updates: Vec<DependencyUpdate>,
    pub(super) workspace_version_removed: bool,
    pub(super) original_workspace_version: Option<Version>,

    pub(super) lockfile_backup: Option<Vec<u8>>,
    pub(super) lockfile_path: Option<PathBuf>,

    pub(super) staged_files: Vec<PathBuf>,
    pub(super) files_were_staged: bool,

    pub(super) commit_result: Option<CommitResult>,

    pub(super) tags_created: Vec<TagResult>,

    pub(super) changesets_deleted: Vec<PathBuf>,
    pub(super) consumed_state: ChangesetConsumedState,
    pub(super) consumed_files_cleared: Vec<ChangesetFileState>,

    #[getset(get, vis = "pub(super)")]
    changelog_backups: Vec<ChangelogFileState>,
    changelogs_written: bool,

    pub(super) version_tracking_writes: Vec<VersionTrackingWrite>,
    pub(super) version_tracking_records: Vec<VersionTrackingWriteRecord>,
}

#[derive(Debug, Clone)]
pub(super) struct ManifestUpdate {
    pub(super) manifest_path: PathBuf,
    pub(super) old_version: Version,
    pub(super) new_version: Version,
    pub(super) written: bool,
    #[allow(dead_code)]
    pub(super) kind: ManifestKind,
}

#[derive(Debug, Clone)]
pub(super) struct DependencyUpdate {
    pub(super) manifest_path: PathBuf,
    pub(super) dependency_name: String,
    pub(super) old_version: Version,
    pub(super) new_version: Version,
}

#[derive(Debug, Clone)]
pub(super) struct VersionTrackingWrite {
    pub(super) manifest_path: PathBuf,
    pub(super) format: ManifestFormat,
    pub(super) version_field_path: String,
    pub(super) new_dependency_version: Version,
}

#[derive(Debug, Clone)]
pub(super) struct VersionTrackingWriteRecord {
    pub(super) manifest_path: PathBuf,
    pub(super) format: ManifestFormat,
    pub(super) version_field_path: String,
    pub(super) old_value: String,
    pub(super) new_version: Version,
    pub(super) written: bool,
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

    pub fn with_additional_packages(
        mut self,
        additional: IndexMap<String, AdditionalManifestInfo>,
    ) -> Self {
        self.additional_package_manifests = additional;
        self
    }

    pub fn with_changelog_backups(mut self, backups: Vec<ChangelogFileState>) -> Self {
        self.changelogs_written = !backups.is_empty();
        self.changelog_backups = backups;
        self
    }

    pub fn with_version_tracking_writes(mut self, writes: Vec<VersionTrackingWrite>) -> Self {
        self.version_tracking_writes = writes;
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
