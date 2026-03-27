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
                .map(|r| (r.name().clone(), r.new_version().clone()))
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
            if let Some(pkg) = ctx.package_lookup.get(release.name()) {
                let changelog_path = pkg.path().join("CHANGELOG.md");
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
            if let Some(pkg) = ctx.package_lookup.get(release.name()) {
                let changelog_path = pkg.path().join("CHANGELOG.md");

                if let Some(version_release) = ctx.aggregator.build_package_release(
                    release.name(),
                    release.new_version(),
                    ctx.today,
                ) {
                    let previous_version = release.current_version().to_string();

                    let result = ctx.changelog_writer.write_release(
                        &changelog_path,
                        &version_release,
                        ctx.repo_info,
                        Some(&previous_version),
                    )?;

                    changelog_updates.push(build_changelog_update(
                        result,
                        release.new_version().clone(),
                        Some(release.name().clone()),
                    ));
                }
            }
        }

        Ok(changelog_updates)
    }
}

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
    ChangelogUpdate::new(result.path().clone(), package, version, result.created())
}

fn max_planned_version(planned_releases: &[PackageVersion]) -> Option<Version> {
    planned_releases
        .iter()
        .map(PackageVersion::new_version)
        .max()
        .cloned()
}

fn max_current_version(planned_releases: &[PackageVersion]) -> Option<Version> {
    planned_releases
        .iter()
        .map(PackageVersion::current_version)
        .max()
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mocks::{MockChangelogWriter, make_changeset, make_package};
    use changeset_core::BumpType;

    fn make_release(name: &str, from: &str, to: &str) -> PackageVersion {
        PackageVersion::new(
            name.to_string(),
            from.parse().expect("valid"),
            to.parse().expect("valid"),
            BumpType::Patch,
            false,
        )
    }

    mod root_changelog_strategy_tests {
        use super::*;
        use std::path::PathBuf;

        #[test]
        fn capture_state_returns_empty_when_no_releases() -> anyhow::Result<()> {
            let writer = MockChangelogWriter::new();
            let lookup = IndexMap::new();
            let ctx = ChangelogCaptureContext {
                project_root: Path::new("/project"),
                planned_releases: &[],
                package_lookup: &lookup,
                changelog_writer: &writer,
            };

            let result = RootChangelogStrategy.capture_state(&ctx)?;

            assert!(result.is_empty());
            Ok(())
        }

        #[test]
        fn capture_state_captures_root_changelog() -> anyhow::Result<()> {
            let writer = MockChangelogWriter::new();
            let releases = vec![make_release("crate-a", "1.0.0", "1.0.1")];
            let lookup = IndexMap::new();
            let ctx = ChangelogCaptureContext {
                project_root: Path::new("/project"),
                planned_releases: &releases,
                package_lookup: &lookup,
                changelog_writer: &writer,
            };

            let result = RootChangelogStrategy.capture_state(&ctx)?;

            assert_eq!(result.len(), 1);
            assert_eq!(result[0].path, PathBuf::from("/project/CHANGELOG.md"));
            assert!(!result[0].file_existed);
            Ok(())
        }

        #[test]
        fn generate_updates_writes_root_changelog() -> anyhow::Result<()> {
            let writer = MockChangelogWriter::new();
            let releases = vec![make_release("crate-a", "1.0.0", "1.0.1")];
            let mut aggregator = ChangesetAggregator::new();
            aggregator.add_changeset(&make_changeset("crate-a", BumpType::Patch, "Fix bug"));
            let lookup = IndexMap::new();
            let ctx = ChangelogGenerateContext {
                project_root: Path::new("/project"),
                aggregator: &aggregator,
                planned_releases: &releases,
                package_lookup: &lookup,
                repo_info: None,
                today: chrono::NaiveDate::from_ymd_opt(2025, 1, 1).expect("valid date"),
                changelog_writer: &writer,
            };

            let updates = RootChangelogStrategy.generate_updates(&ctx)?;

            assert_eq!(updates.len(), 1);
            assert!(updates[0].package().is_none());
            Ok(())
        }
    }

    mod strategy_for_tests {
        use super::*;
        use indexmap::IndexMap;
        use std::path::{Path, PathBuf};

        #[test]
        fn root_location_dispatches_to_root_strategy() -> anyhow::Result<()> {
            let strategy = strategy_for(ChangelogLocation::Root);
            let releases = vec![make_release("crate-a", "1.0.0", "1.0.1")];
            let lookup = IndexMap::new();
            let writer = MockChangelogWriter::new();
            let ctx = ChangelogCaptureContext {
                project_root: Path::new("/project"),
                planned_releases: &releases,
                package_lookup: &lookup,
                changelog_writer: &writer,
            };

            let result = strategy.capture_state(&ctx)?;

            assert_eq!(result.len(), 1);
            assert_eq!(result[0].path, PathBuf::from("/project/CHANGELOG.md"));
            Ok(())
        }

        #[test]
        fn per_package_location_dispatches_to_per_package_strategy() -> anyhow::Result<()> {
            let strategy = strategy_for(ChangelogLocation::PerPackage);
            let releases = vec![make_release("crate-a", "1.0.0", "1.0.1")];
            let pkg = make_package("crate-a", "1.0.0");
            let mut lookup = IndexMap::new();
            lookup.insert("crate-a".to_string(), pkg);
            let writer = MockChangelogWriter::new();
            let ctx = ChangelogCaptureContext {
                project_root: Path::new("/project"),
                planned_releases: &releases,
                package_lookup: &lookup,
                changelog_writer: &writer,
            };

            let result = strategy.capture_state(&ctx)?;

            assert_eq!(result.len(), 1);
            assert!(result[0].path.to_string_lossy().contains("crate-a"));
            Ok(())
        }
    }

    mod per_package_changelog_strategy_tests {
        use super::*;

        #[test]
        fn capture_state_returns_per_package_paths() -> anyhow::Result<()> {
            let writer = MockChangelogWriter::new();
            let releases = vec![make_release("crate-a", "1.0.0", "1.0.1")];
            let pkg = make_package("crate-a", "1.0.0");
            let mut lookup = IndexMap::new();
            lookup.insert("crate-a".to_string(), pkg);
            let ctx = ChangelogCaptureContext {
                project_root: Path::new("/project"),
                planned_releases: &releases,
                package_lookup: &lookup,
                changelog_writer: &writer,
            };

            let result = PerPackageChangelogStrategy.capture_state(&ctx)?;

            assert_eq!(result.len(), 1);
            assert!(result[0].path.to_string_lossy().contains("crate-a"));
            Ok(())
        }

        #[test]
        fn generate_updates_writes_per_package_changelog() -> anyhow::Result<()> {
            let writer = MockChangelogWriter::new();
            let releases = vec![make_release("crate-a", "1.0.0", "1.0.1")];
            let pkg = make_package("crate-a", "1.0.0");
            let mut aggregator = ChangesetAggregator::new();
            aggregator.add_changeset(&make_changeset("crate-a", BumpType::Patch, "Fix bug"));
            let mut lookup = IndexMap::new();
            lookup.insert("crate-a".to_string(), pkg);
            let ctx = ChangelogGenerateContext {
                project_root: Path::new("/project"),
                aggregator: &aggregator,
                planned_releases: &releases,
                package_lookup: &lookup,
                repo_info: None,
                today: chrono::NaiveDate::from_ymd_opt(2025, 1, 1).expect("valid date"),
                changelog_writer: &writer,
            };

            let updates = PerPackageChangelogStrategy.generate_updates(&ctx)?;

            assert_eq!(updates.len(), 1);
            assert_eq!(updates[0].package().map(String::as_str), Some("crate-a"));
            Ok(())
        }

        #[test]
        fn generate_updates_returns_empty_when_no_changes() -> anyhow::Result<()> {
            let writer = MockChangelogWriter::new();
            let releases = vec![make_release("crate-a", "1.0.0", "1.0.1")];
            let pkg = make_package("crate-a", "1.0.0");
            let aggregator = ChangesetAggregator::new();
            let mut lookup = IndexMap::new();
            lookup.insert("crate-a".to_string(), pkg);
            let ctx = ChangelogGenerateContext {
                project_root: Path::new("/project"),
                aggregator: &aggregator,
                planned_releases: &releases,
                package_lookup: &lookup,
                repo_info: None,
                today: chrono::NaiveDate::from_ymd_opt(2025, 1, 1).expect("valid date"),
                changelog_writer: &writer,
            };

            let updates = PerPackageChangelogStrategy.generate_updates(&ctx)?;

            assert!(updates.is_empty());
            Ok(())
        }
    }

    mod changelog_file_read_error {
        use super::*;
        use std::path::PathBuf;

        #[test]
        fn capture_state_returns_changelog_file_read_when_read_fails() {
            let changelog_path = PathBuf::from("/nonexistent/project/CHANGELOG.md");
            let writer = MockChangelogWriter::new().with_existing_changelog(changelog_path);
            let releases = vec![make_release("crate-a", "1.0.0", "1.0.1")];
            let lookup = IndexMap::new();
            let ctx = ChangelogCaptureContext {
                project_root: Path::new("/nonexistent/project"),
                planned_releases: &releases,
                package_lookup: &lookup,
                changelog_writer: &writer,
            };

            let result = RootChangelogStrategy.capture_state(&ctx);

            assert!(result.is_err());
            assert!(matches!(
                result.expect_err("should fail"),
                OperationError::ChangelogFileRead { .. }
            ),);
        }
    }

    mod version_helpers {
        use super::*;

        #[test]
        fn max_planned_version_empty_returns_none() {
            assert!(max_planned_version(&[]).is_none());
        }

        #[test]
        fn max_planned_version_single_element() {
            let releases = vec![make_release("crate-a", "1.0.0", "1.0.1")];

            let result = max_planned_version(&releases);

            assert_eq!(result, Some("1.0.1".parse().expect("valid version")));
        }

        #[test]
        fn max_planned_version_multiple_returns_max() {
            let releases = vec![
                make_release("crate-a", "1.0.0", "1.0.1"),
                make_release("crate-b", "2.0.0", "3.0.0"),
                make_release("crate-c", "0.5.0", "0.6.0"),
            ];

            let result = max_planned_version(&releases);

            assert_eq!(result, Some("3.0.0".parse().expect("valid version")));
        }

        #[test]
        fn max_current_version_empty_returns_none() {
            assert!(max_current_version(&[]).is_none());
        }

        #[test]
        fn max_current_version_single_element() {
            let releases = vec![make_release("crate-a", "1.0.0", "1.0.1")];

            let result = max_current_version(&releases);

            assert_eq!(result, Some("1.0.0".parse().expect("valid version")));
        }

        #[test]
        fn max_current_version_multiple_returns_max() {
            let releases = vec![
                make_release("crate-a", "1.0.0", "1.0.1"),
                make_release("crate-b", "2.0.0", "3.0.0"),
                make_release("crate-c", "0.5.0", "0.6.0"),
            ];

            let result = max_current_version(&releases);

            assert_eq!(result, Some("2.0.0".parse().expect("valid version")));
        }
    }
}
