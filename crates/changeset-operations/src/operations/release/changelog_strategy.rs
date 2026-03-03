use std::path::Path;

use changeset_changelog::{ChangelogLocation, RepositoryInfo};
use changeset_core::PackageInfo;
use chrono::NaiveDate;
use indexmap::IndexMap;
use semver::Version;

use super::types::{ChangelogFileState, ChangelogUpdate};
use crate::Result;
use crate::error::OperationError;
use crate::operations::changelog_aggregation::ChangesetAggregator;
use crate::traits::{ChangelogWriteResult, ChangelogWriter};
use crate::types::PackageVersion;

pub(super) struct ChangelogCaptureContext<'a> {
    pub(super) project_root: &'a Path,
    pub(super) planned_releases: &'a [PackageVersion],
    pub(super) package_lookup: &'a IndexMap<String, PackageInfo>,
    pub(super) changelog_writer: &'a dyn ChangelogWriter,
}

pub(super) struct ChangelogGenerateContext<'a> {
    pub(super) project_root: &'a Path,
    pub(super) aggregator: &'a ChangesetAggregator,
    pub(super) planned_releases: &'a [PackageVersion],
    pub(super) package_lookup: &'a IndexMap<String, PackageInfo>,
    pub(super) repo_info: Option<&'a RepositoryInfo>,
    pub(super) today: NaiveDate,
    pub(super) changelog_writer: &'a dyn ChangelogWriter,
}

pub(super) trait ChangelogHandler {
    fn capture_state(&self, ctx: &ChangelogCaptureContext<'_>) -> Result<Vec<ChangelogFileState>>;

    fn generate_updates(&self, ctx: &ChangelogGenerateContext<'_>) -> Result<Vec<ChangelogUpdate>>;
}

struct RootChangelogStrategy;
struct PerPackageChangelogStrategy;

pub(super) fn strategy_for(location: ChangelogLocation) -> Box<dyn ChangelogHandler> {
    match location {
        ChangelogLocation::Root => Box::new(RootChangelogStrategy),
        ChangelogLocation::PerPackage => Box::new(PerPackageChangelogStrategy),
    }
}

fn read_changelog_content(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).map_err(|source| OperationError::ChangelogFileRead {
        path: path.to_path_buf(),
        source,
    })
}

fn build_changelog_update(
    result: ChangelogWriteResult,
    version: Version,
    package: Option<String>,
) -> ChangelogUpdate {
    ChangelogUpdate {
        path: result.path,
        package,
        version,
        created: result.created,
    }
}

fn max_planned_version(planned_releases: &[PackageVersion]) -> Option<Version> {
    planned_releases
        .iter()
        .map(|r| &r.new_version)
        .max()
        .cloned()
}

fn max_current_version(planned_releases: &[PackageVersion]) -> Option<Version> {
    planned_releases
        .iter()
        .map(|r| &r.current_version)
        .max()
        .cloned()
}

impl ChangelogHandler for RootChangelogStrategy {
    fn capture_state(&self, ctx: &ChangelogCaptureContext<'_>) -> Result<Vec<ChangelogFileState>> {
        let mut backups = Vec::new();
        let changelog_path = ctx.project_root.join("CHANGELOG.md");

        if max_planned_version(ctx.planned_releases).is_some() {
            let file_existed = ctx.changelog_writer.changelog_exists(&changelog_path);
            let original_content = if file_existed {
                Some(read_changelog_content(&changelog_path)?)
            } else {
                None
            };

            backups.push(ChangelogFileState {
                path: changelog_path,
                original_content,
                file_existed,
            });
        }

        Ok(backups)
    }

    fn generate_updates(&self, ctx: &ChangelogGenerateContext<'_>) -> Result<Vec<ChangelogUpdate>> {
        let mut changelog_updates = Vec::new();
        let changelog_path = ctx.project_root.join("CHANGELOG.md");

        if let Some(version) = max_planned_version(ctx.planned_releases) {
            let packages: Vec<_> = ctx
                .planned_releases
                .iter()
                .map(|r| (r.name.clone(), r.new_version.clone()))
                .collect();

            if let Some(release) = ctx
                .aggregator
                .build_root_release(&version, ctx.today, &packages)
            {
                let previous_tag = max_current_version(ctx.planned_releases).map(|v| v.to_string());

                let result = ctx.changelog_writer.write_release(
                    &changelog_path,
                    &release,
                    ctx.repo_info,
                    previous_tag.as_deref(),
                )?;

                changelog_updates.push(build_changelog_update(result, version, None));
            }
        }

        Ok(changelog_updates)
    }
}

impl ChangelogHandler for PerPackageChangelogStrategy {
    fn capture_state(&self, ctx: &ChangelogCaptureContext<'_>) -> Result<Vec<ChangelogFileState>> {
        let mut backups = Vec::new();

        for release in ctx.planned_releases {
            if let Some(pkg) = ctx.package_lookup.get(&release.name) {
                let changelog_path = pkg.path.join("CHANGELOG.md");
                let file_existed = ctx.changelog_writer.changelog_exists(&changelog_path);
                let original_content = if file_existed {
                    Some(read_changelog_content(&changelog_path)?)
                } else {
                    None
                };

                backups.push(ChangelogFileState {
                    path: changelog_path,
                    original_content,
                    file_existed,
                });
            }
        }

        Ok(backups)
    }

    fn generate_updates(&self, ctx: &ChangelogGenerateContext<'_>) -> Result<Vec<ChangelogUpdate>> {
        let mut changelog_updates = Vec::new();

        for release in ctx.planned_releases {
            if let Some(pkg) = ctx.package_lookup.get(&release.name) {
                let changelog_path = pkg.path.join("CHANGELOG.md");

                if let Some(version_release) = ctx.aggregator.build_package_release(
                    &release.name,
                    &release.new_version,
                    ctx.today,
                ) {
                    let previous_version = release.current_version.to_string();

                    let result = ctx.changelog_writer.write_release(
                        &changelog_path,
                        &version_release,
                        ctx.repo_info,
                        Some(&previous_version),
                    )?;

                    changelog_updates.push(build_changelog_update(
                        result,
                        release.new_version.clone(),
                        Some(release.name.clone()),
                    ));
                }
            }
        }

        Ok(changelog_updates)
    }
}
