use std::collections::HashMap;
use std::path::PathBuf;

use changeset_core::{PackageInfo, PrereleaseSpec};
use changeset_project::GraduationState;
use indexmap::IndexMap;
use semver::Version;

use crate::types::{PackageReleaseConfig, PackageVersion};

pub struct ReleaseInput {
    pub dry_run: bool,
    pub convert_inherited: bool,
    pub no_commit: bool,
    pub no_tags: bool,
    pub keep_changesets: bool,
    pub force: bool,
    pub per_package_config: HashMap<String, PackageReleaseConfig>,
    pub global_prerelease: Option<PrereleaseSpec>,
    pub graduate_all: bool,
}

#[derive(Debug, Clone)]
pub struct ChangelogUpdate {
    pub path: PathBuf,
    pub package: Option<String>,
    pub version: Version,
    pub created: bool,
}

#[derive(Debug, Clone)]
pub struct CommitResult {
    pub sha: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct TagResult {
    pub name: String,
    pub target_sha: String,
}

#[derive(Debug, Clone, Default)]
pub struct GitOperationResult {
    pub commit: Option<CommitResult>,
    pub tags_created: Vec<TagResult>,
    pub changesets_deleted: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ReleaseOutput {
    pub planned_releases: Vec<PackageVersion>,
    pub unchanged_packages: Vec<String>,
    pub changesets_consumed: Vec<PathBuf>,
    pub changelog_updates: Vec<ChangelogUpdate>,
    pub git_result: Option<GitOperationResult>,
}

#[derive(Debug)]
pub enum ReleaseOutcome {
    DryRun(ReleaseOutput),
    Executed(ReleaseOutput),
    NoChangesets,
}

pub(super) struct GitOptions {
    pub(super) should_commit: bool,
    pub(super) should_create_tags: bool,
    pub(super) should_delete_changesets: bool,
}

pub(super) enum PrepareResult {
    Ready(ReleaseContext),
    EarlyReturn(ReleaseOutcome),
}

pub(super) struct ReleaseContext {
    pub(super) project: changeset_project::CargoProject,
    pub(super) root_config: changeset_project::RootChangesetConfig,
    pub(super) changeset_dir: PathBuf,
    pub(super) changeset_files: Vec<PathBuf>,
    pub(super) prerelease_state: Option<changeset_project::PrereleaseState>,
    pub(super) graduation_state: Option<GraduationState>,
    pub(super) per_package_config: HashMap<String, PackageReleaseConfig>,
    pub(super) is_prerelease_graduation: bool,
    pub(super) is_graduating: bool,
    pub(super) is_prerelease_release: bool,
    pub(super) git_options: GitOptions,
    pub(super) inherited_packages: Vec<String>,
}

pub(super) struct ReleasePlan {
    pub(super) output: ReleaseOutput,
    pub(super) package_lookup: IndexMap<String, PackageInfo>,
    pub(super) changelog_backups: Vec<super::steps::ChangelogFileState>,
}
