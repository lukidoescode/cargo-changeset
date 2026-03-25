use std::collections::HashMap;
use std::path::PathBuf;

use changeset_core::{Changeset, PackageInfo, PrereleaseSpec};
use changeset_project::GraduationState;
use derive_builder::Builder;
use gset::Getset;
use indexmap::IndexMap;
use semver::Version;

use crate::types::{PackageReleaseConfig, PackageVersion};

#[derive(Builder, Getset, Default)]
#[builder(default)]
pub struct ReleaseInput {
    #[getset(get_copy, vis = "pub")]
    dry_run: bool,
    #[getset(get_copy, vis = "pub")]
    convert_inherited: bool,
    #[getset(get_copy, vis = "pub")]
    no_commit: bool,
    #[getset(get_copy, vis = "pub")]
    no_tags: bool,
    #[getset(get_copy, vis = "pub")]
    keep_changesets: bool,
    #[getset(get_copy, vis = "pub")]
    force: bool,
    #[getset(get, vis = "pub")]
    per_package_config: HashMap<String, PackageReleaseConfig>,
    #[getset(get_as_ref, vis = "pub", ty = "Option<&PrereleaseSpec>")]
    global_prerelease: Option<PrereleaseSpec>,
    #[getset(get_copy, vis = "pub")]
    graduate_all: bool,
}

#[derive(Debug, Clone, Getset)]
pub struct ChangelogUpdate {
    #[getset(get, vis = "pub")]
    path: PathBuf,
    #[getset(get_as_ref, vis = "pub", ty = "Option<&String>")]
    package: Option<String>,
    #[getset(get, vis = "pub")]
    version: Version,
    #[getset(get_copy, vis = "pub")]
    created: bool,
}

impl ChangelogUpdate {
    pub(crate) fn new(
        path: PathBuf,
        package: Option<String>,
        version: Version,
        created: bool,
    ) -> Self {
        Self {
            path,
            package,
            version,
            created,
        }
    }
}

#[derive(Debug, Clone, Getset)]
pub struct CommitResult {
    #[getset(get, vis = "pub")]
    sha: String,
    #[getset(get, vis = "pub")]
    message: String,
}

impl CommitResult {
    pub(crate) fn new(sha: String, message: String) -> Self {
        Self { sha, message }
    }
}

#[derive(Debug, Clone, Getset)]
pub struct TagResult {
    #[getset(get, vis = "pub")]
    name: String,
    #[getset(get, vis = "pub")]
    target_sha: String,
}

impl TagResult {
    pub(crate) fn new(name: String, target_sha: String) -> Self {
        Self { name, target_sha }
    }
}

#[derive(Debug, Clone, Default, Getset)]
pub struct GitOperationResult {
    #[getset(get_as_ref, vis = "pub", ty = "Option<&CommitResult>")]
    commit: Option<CommitResult>,
    #[getset(get, vis = "pub")]
    tags_created: Vec<TagResult>,
    #[getset(get, vis = "pub")]
    changesets_deleted: Vec<PathBuf>,
}

impl GitOperationResult {
    pub(crate) fn new(
        commit: Option<CommitResult>,
        tags_created: Vec<TagResult>,
        changesets_deleted: Vec<PathBuf>,
    ) -> Self {
        Self {
            commit,
            tags_created,
            changesets_deleted,
        }
    }
}

#[derive(Debug, Clone, Getset)]
pub struct ReleaseOutput {
    #[getset(get, vis = "pub")]
    planned_releases: Vec<PackageVersion>,
    #[getset(get, vis = "pub")]
    unchanged_packages: Vec<String>,
    #[getset(get, vis = "pub")]
    changesets_consumed: Vec<PathBuf>,
    #[getset(get, vis = "pub")]
    changelog_updates: Vec<ChangelogUpdate>,
    #[getset(get_as_ref, vis = "pub", ty = "Option<&GitOperationResult>")]
    git_result: Option<GitOperationResult>,
}

impl ReleaseOutput {
    pub(crate) fn new(
        planned_releases: Vec<PackageVersion>,
        unchanged_packages: Vec<String>,
        changesets_consumed: Vec<PathBuf>,
        changelog_updates: Vec<ChangelogUpdate>,
        git_result: Option<GitOperationResult>,
    ) -> Self {
        Self {
            planned_releases,
            unchanged_packages,
            changesets_consumed,
            changelog_updates,
            git_result,
        }
    }

    pub(super) fn with_git_result(self, git_result: GitOperationResult) -> Self {
        Self {
            git_result: Some(git_result),
            ..self
        }
    }
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
    Ready(Box<ReleaseContext>),
    EarlyReturn(ReleaseOutcome),
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ReleaseClassification {
    pub(super) is_prerelease_graduation: bool,
    pub(super) is_graduating: bool,
    pub(super) is_prerelease_release: bool,
}

pub(super) struct ReleaseContext {
    pub(super) project: changeset_project::CargoProject,
    pub(super) root_config: changeset_project::RootChangesetConfig,
    pub(super) changeset_dir: PathBuf,
    pub(super) changeset_files: Vec<PathBuf>,
    pub(super) prerelease_state: Option<changeset_project::PrereleaseState>,
    pub(super) graduation_state: Option<GraduationState>,
    pub(super) per_package_config: HashMap<String, PackageReleaseConfig>,
    pub(super) classification: ReleaseClassification,
    pub(super) git_options: GitOptions,
    pub(super) inherited_packages: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ChangelogFileState {
    pub(crate) path: PathBuf,
    pub(crate) original_content: Option<String>,
    pub(crate) file_existed: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct ChangesetFileState {
    pub(crate) path: PathBuf,
    pub(crate) original_consumed_status: Option<String>,
    pub(crate) backup: Option<Changeset>,
}

pub(super) struct ReleasePlan {
    pub(super) output: ReleaseOutput,
    pub(super) package_lookup: IndexMap<String, PackageInfo>,
    pub(super) changelog_backups: Vec<ChangelogFileState>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PackageReleaseConfigBuilder;
    use changeset_core::PrereleaseSpec;
    use std::collections::HashMap;

    #[test]
    fn builder_defaults_all_false() {
        let input = ReleaseInputBuilder::default()
            .build()
            .expect("all fields have defaults");

        assert!(!input.dry_run());
        assert!(!input.convert_inherited());
        assert!(!input.no_commit());
        assert!(!input.no_tags());
        assert!(!input.keep_changesets());
        assert!(!input.force());
        assert!(!input.graduate_all());
        assert!(input.per_package_config().is_empty());
        assert!(input.global_prerelease().is_none());
    }

    #[test]
    fn builder_sets_dry_run() {
        let input = ReleaseInputBuilder::default()
            .dry_run(true)
            .build()
            .expect("all fields have defaults");

        assert!(input.dry_run());
    }

    #[test]
    fn builder_sets_global_prerelease() {
        let input = ReleaseInputBuilder::default()
            .global_prerelease(Some(PrereleaseSpec::Alpha))
            .build()
            .expect("all fields have defaults");

        let prerelease = input.global_prerelease();
        assert!(prerelease.is_some());
        assert_eq!(
            prerelease.expect("should have prerelease").identifier(),
            "alpha"
        );
    }

    #[test]
    fn builder_sets_per_package_config() {
        let mut map = HashMap::new();
        map.insert(
            "crate-a".to_string(),
            PackageReleaseConfigBuilder::default()
                .build()
                .expect("all fields have defaults"),
        );

        let input = ReleaseInputBuilder::default()
            .per_package_config(map)
            .build()
            .expect("all fields have defaults");

        assert!(input.per_package_config().contains_key("crate-a"));
    }
}
