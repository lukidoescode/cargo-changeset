use std::collections::HashMap;
use std::path::PathBuf;

use changeset_core::{Changeset, PackageInfo, PrereleaseSpec};
use changeset_project::GraduationState;
use indexmap::IndexMap;
use semver::Version;

use crate::types::{PackageReleaseConfig, PackageVersion};

pub struct ReleaseInput {
    dry_run: bool,
    convert_inherited: bool,
    no_commit: bool,
    no_tags: bool,
    keep_changesets: bool,
    force: bool,
    per_package_config: HashMap<String, PackageReleaseConfig>,
    global_prerelease: Option<PrereleaseSpec>,
    graduate_all: bool,
}

impl ReleaseInput {
    #[must_use]
    pub fn builder() -> ReleaseInputBuilder {
        ReleaseInputBuilder::new()
    }

    #[must_use]
    pub fn dry_run(&self) -> bool {
        self.dry_run
    }

    #[must_use]
    pub fn convert_inherited(&self) -> bool {
        self.convert_inherited
    }

    #[must_use]
    pub fn no_commit(&self) -> bool {
        self.no_commit
    }

    #[must_use]
    pub fn no_tags(&self) -> bool {
        self.no_tags
    }

    #[must_use]
    pub fn keep_changesets(&self) -> bool {
        self.keep_changesets
    }

    #[must_use]
    pub fn force(&self) -> bool {
        self.force
    }

    #[must_use]
    pub fn per_package_config(&self) -> &HashMap<String, PackageReleaseConfig> {
        &self.per_package_config
    }

    #[must_use]
    pub fn global_prerelease(&self) -> Option<&PrereleaseSpec> {
        self.global_prerelease.as_ref()
    }

    #[must_use]
    pub fn graduate_all(&self) -> bool {
        self.graduate_all
    }
}

#[derive(Default)]
pub struct ReleaseInputBuilder {
    dry_run: bool,
    convert_inherited: bool,
    no_commit: bool,
    no_tags: bool,
    keep_changesets: bool,
    force: bool,
    per_package_config: HashMap<String, PackageReleaseConfig>,
    global_prerelease: Option<PrereleaseSpec>,
    graduate_all: bool,
}

impl ReleaseInputBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn dry_run(mut self, v: bool) -> Self {
        self.dry_run = v;
        self
    }

    #[must_use]
    pub fn convert_inherited(mut self, v: bool) -> Self {
        self.convert_inherited = v;
        self
    }

    #[must_use]
    pub fn no_commit(mut self, v: bool) -> Self {
        self.no_commit = v;
        self
    }

    #[must_use]
    pub fn no_tags(mut self, v: bool) -> Self {
        self.no_tags = v;
        self
    }

    #[must_use]
    pub fn keep_changesets(mut self, v: bool) -> Self {
        self.keep_changesets = v;
        self
    }

    #[must_use]
    pub fn force(mut self, v: bool) -> Self {
        self.force = v;
        self
    }

    #[must_use]
    pub fn per_package_config(mut self, v: HashMap<String, PackageReleaseConfig>) -> Self {
        self.per_package_config = v;
        self
    }

    #[must_use]
    pub fn global_prerelease(mut self, v: Option<PrereleaseSpec>) -> Self {
        self.global_prerelease = v;
        self
    }

    #[must_use]
    pub fn graduate_all(mut self, v: bool) -> Self {
        self.graduate_all = v;
        self
    }

    #[must_use]
    pub fn build(self) -> ReleaseInput {
        ReleaseInput {
            dry_run: self.dry_run,
            convert_inherited: self.convert_inherited,
            no_commit: self.no_commit,
            no_tags: self.no_tags,
            keep_changesets: self.keep_changesets,
            force: self.force,
            per_package_config: self.per_package_config,
            global_prerelease: self.global_prerelease,
            graduate_all: self.graduate_all,
        }
    }
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
    use crate::types::PackageReleaseConfig;
    use changeset_core::PrereleaseSpec;
    use std::collections::HashMap;

    #[test]
    fn builder_defaults_all_false() {
        let input = ReleaseInput::builder().build();

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
        let input = ReleaseInput::builder().dry_run(true).build();

        assert!(input.dry_run());
    }

    #[test]
    fn builder_sets_global_prerelease() {
        let input = ReleaseInput::builder()
            .global_prerelease(Some(PrereleaseSpec::Alpha))
            .build();

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
            PackageReleaseConfig {
                prerelease: None,
                graduate_zero: false,
            },
        );

        let input = ReleaseInput::builder().per_package_config(map).build();

        assert!(input.per_package_config().contains_key("crate-a"));
    }
}
