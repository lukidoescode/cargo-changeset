use std::collections::HashSet;

use changeset_core::{BumpType, Changeset, PackageInfo};
use changeset_version::{bump_version, max_bump_type};
use indexmap::IndexMap;

use super::release::PackageVersion;

/// Result of planning version releases from changesets.
#[derive(Debug, Clone)]
pub struct ReleasePlan {
    /// Calculated package versions for release.
    pub releases: Vec<PackageVersion>,
    /// Packages referenced in changesets but not found in workspace.
    pub unknown_packages: Vec<String>,
}

/// Plans version releases by aggregating changesets and calculating new versions.
pub struct VersionPlanner;

impl VersionPlanner {
    #[must_use]
    pub fn plan_releases(changesets: &[Changeset], packages: &[PackageInfo]) -> ReleasePlan {
        let package_lookup: IndexMap<_, _> = packages.iter().map(|p| (p.name.clone(), p)).collect();
        let bumps_by_package = Self::aggregate_bumps(changesets);

        let mut releases = Vec::new();
        let mut unknown_packages = Vec::new();

        for (name, bumps) in &bumps_by_package {
            let Some(bump_type) = max_bump_type(bumps) else {
                continue;
            };

            if let Some(pkg) = package_lookup.get(name) {
                let new_version = bump_version(&pkg.version, bump_type);
                releases.push(PackageVersion {
                    name: name.clone(),
                    current_version: pkg.version.clone(),
                    new_version,
                    bump_type,
                });
            } else {
                unknown_packages.push(name.clone());
            }
        }

        ReleasePlan {
            releases,
            unknown_packages,
        }
    }

    #[must_use]
    pub fn aggregate_bumps(changesets: &[Changeset]) -> IndexMap<String, Vec<BumpType>> {
        let mut bumps_by_package: IndexMap<String, Vec<BumpType>> = IndexMap::new();

        for changeset in changesets {
            for release in &changeset.releases {
                bumps_by_package
                    .entry(release.name.clone())
                    .or_default()
                    .push(release.bump_type);
            }
        }

        bumps_by_package
    }

    /// Identifies packages that have changesets and those without.
    #[must_use]
    pub fn partition_packages(
        changesets: &[Changeset],
        packages: &[PackageInfo],
    ) -> (HashSet<String>, Vec<PackageInfo>) {
        let packages_with_changesets: HashSet<String> = changesets
            .iter()
            .flat_map(|c| c.releases.iter().map(|r| r.name.clone()))
            .collect();

        let unchanged_packages: Vec<PackageInfo> = packages
            .iter()
            .filter(|p| !packages_with_changesets.contains(&p.name))
            .cloned()
            .collect();

        (packages_with_changesets, unchanged_packages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use changeset_core::{ChangeCategory, PackageRelease};
    use semver::Version;
    use std::path::PathBuf;

    fn make_package(name: &str, version: &str) -> PackageInfo {
        PackageInfo {
            name: name.to_string(),
            version: version.parse().expect("valid version"),
            path: PathBuf::from(format!("/mock/crates/{name}")),
        }
    }

    fn make_changeset(package_name: &str, bump: BumpType, summary: &str) -> Changeset {
        Changeset {
            summary: summary.to_string(),
            releases: vec![PackageRelease {
                name: package_name.to_string(),
                bump_type: bump,
            }],
            category: ChangeCategory::Changed,
        }
    }

    fn make_multi_changeset(releases: Vec<(&str, BumpType)>, summary: &str) -> Changeset {
        Changeset {
            summary: summary.to_string(),
            releases: releases
                .into_iter()
                .map(|(name, bump)| PackageRelease {
                    name: name.to_string(),
                    bump_type: bump,
                })
                .collect(),
            category: ChangeCategory::Changed,
        }
    }

    #[test]
    fn plan_releases_empty_changesets_returns_empty_plan() {
        let packages = vec![make_package("my-crate", "1.0.0")];

        let plan = VersionPlanner::plan_releases(&[], &packages);

        assert!(plan.releases.is_empty());
        assert!(plan.unknown_packages.is_empty());
    }

    #[test]
    fn plan_releases_single_package_single_bump() {
        let packages = vec![make_package("my-crate", "1.0.0")];
        let changesets = vec![make_changeset("my-crate", BumpType::Patch, "Fix bug")];

        let plan = VersionPlanner::plan_releases(&changesets, &packages);

        assert_eq!(plan.releases.len(), 1);
        assert!(plan.unknown_packages.is_empty());

        let release = &plan.releases[0];
        assert_eq!(release.name, "my-crate");
        assert_eq!(release.current_version, Version::new(1, 0, 0));
        assert_eq!(release.new_version, Version::new(1, 0, 1));
        assert_eq!(release.bump_type, BumpType::Patch);
    }

    #[test]
    fn plan_releases_single_package_takes_max_bump() {
        let packages = vec![make_package("my-crate", "1.0.0")];
        let changesets = vec![
            make_changeset("my-crate", BumpType::Patch, "Fix bug"),
            make_changeset("my-crate", BumpType::Minor, "Add feature"),
            make_changeset("my-crate", BumpType::Patch, "Another fix"),
        ];

        let plan = VersionPlanner::plan_releases(&changesets, &packages);

        assert_eq!(plan.releases.len(), 1);
        let release = &plan.releases[0];
        assert_eq!(release.new_version, Version::new(1, 1, 0));
        assert_eq!(release.bump_type, BumpType::Minor);
    }

    #[test]
    fn plan_releases_multiple_packages_independent_bumps() {
        let packages = vec![
            make_package("crate-a", "1.0.0"),
            make_package("crate-b", "2.5.3"),
        ];
        let changesets = vec![
            make_changeset("crate-a", BumpType::Minor, "Add feature to A"),
            make_changeset("crate-b", BumpType::Major, "Breaking change in B"),
        ];

        let plan = VersionPlanner::plan_releases(&changesets, &packages);

        assert_eq!(plan.releases.len(), 2);
        assert!(plan.unknown_packages.is_empty());

        let release_a = plan
            .releases
            .iter()
            .find(|r| r.name == "crate-a")
            .expect("crate-a should be in releases");
        assert_eq!(release_a.new_version, Version::new(1, 1, 0));

        let release_b = plan
            .releases
            .iter()
            .find(|r| r.name == "crate-b")
            .expect("crate-b should be in releases");
        assert_eq!(release_b.new_version, Version::new(3, 0, 0));
    }

    #[test]
    fn plan_releases_unknown_package_collected_not_errored() {
        let packages = vec![make_package("known-crate", "1.0.0")];
        let changesets = vec![make_changeset("unknown-crate", BumpType::Patch, "Fix")];

        let plan = VersionPlanner::plan_releases(&changesets, &packages);

        assert!(plan.releases.is_empty());
        assert_eq!(plan.unknown_packages, vec!["unknown-crate"]);
    }

    #[test]
    fn plan_releases_mixed_known_and_unknown_packages() {
        let packages = vec![make_package("known-crate", "1.0.0")];
        let changesets = vec![make_multi_changeset(
            vec![
                ("known-crate", BumpType::Minor),
                ("unknown-crate", BumpType::Patch),
            ],
            "Mixed changes",
        )];

        let plan = VersionPlanner::plan_releases(&changesets, &packages);

        assert_eq!(plan.releases.len(), 1);
        assert_eq!(plan.releases[0].name, "known-crate");
        assert_eq!(plan.unknown_packages, vec!["unknown-crate"]);
    }

    #[test]
    fn aggregate_bumps_collects_all_bump_types() {
        let changesets = vec![
            make_changeset("crate-a", BumpType::Patch, "Fix"),
            make_changeset("crate-a", BumpType::Minor, "Feature"),
            make_changeset("crate-b", BumpType::Major, "Breaking"),
        ];

        let bumps = VersionPlanner::aggregate_bumps(&changesets);

        assert_eq!(bumps["crate-a"], vec![BumpType::Patch, BumpType::Minor]);
        assert_eq!(bumps["crate-b"], vec![BumpType::Major]);
    }

    #[test]
    fn partition_packages_identifies_changed_and_unchanged() {
        let packages = vec![
            make_package("changed", "1.0.0"),
            make_package("unchanged", "2.0.0"),
        ];
        let changesets = vec![make_changeset("changed", BumpType::Patch, "Fix")];

        let (with_changesets, without) = VersionPlanner::partition_packages(&changesets, &packages);

        assert!(with_changesets.contains("changed"));
        assert!(!with_changesets.contains("unchanged"));
        assert_eq!(without.len(), 1);
        assert_eq!(without[0].name, "unchanged");
    }

    #[test]
    fn plan_releases_handles_prerelease_versions() {
        let packages = vec![make_package("my-crate", "1.0.0-alpha.1")];
        let changesets = vec![make_changeset("my-crate", BumpType::Patch, "Fix")];

        let plan = VersionPlanner::plan_releases(&changesets, &packages);

        assert_eq!(plan.releases.len(), 1);
        let release = &plan.releases[0];
        // Pre-release versions should be bumped according to semver rules
        // changeset_version::bump_version handles this
        assert_eq!(
            release.current_version,
            "1.0.0-alpha.1".parse::<Version>().expect("valid")
        );
        // The actual new version depends on changeset_version implementation
        // but should be a valid version greater than the original
        assert!(release.new_version > release.current_version);
    }

    #[test]
    fn plan_releases_handles_build_metadata() {
        let packages = vec![make_package("my-crate", "1.0.0+build.123")];
        let changesets = vec![make_changeset("my-crate", BumpType::Minor, "Feature")];

        let plan = VersionPlanner::plan_releases(&changesets, &packages);

        assert_eq!(plan.releases.len(), 1);
        let release = &plan.releases[0];
        assert_eq!(
            release.current_version,
            "1.0.0+build.123".parse::<Version>().expect("valid")
        );
    }

    #[test]
    fn plan_releases_with_zero_major_version() {
        let packages = vec![make_package("my-crate", "0.1.0")];
        let changesets = vec![make_changeset("my-crate", BumpType::Major, "Breaking")];

        let plan = VersionPlanner::plan_releases(&changesets, &packages);

        assert_eq!(plan.releases.len(), 1);
        let release = &plan.releases[0];
        assert_eq!(release.current_version, Version::new(0, 1, 0));
        assert_eq!(release.new_version, Version::new(1, 0, 0));
    }
}
