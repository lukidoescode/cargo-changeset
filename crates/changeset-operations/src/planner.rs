use std::collections::{HashMap, HashSet};

use changeset_core::{BumpType, Changeset, PackageInfo, PrereleaseSpec, ZeroVersionBehavior};
use changeset_version::{
    VersionError, calculate_new_version, calculate_new_version_with_zero_behavior, is_zero_version,
    max_bump_type,
};
use indexmap::IndexMap;

use crate::types::{PackageReleaseConfig, PackageVersion};

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
    /// Plans version releases based on changesets.
    ///
    /// # Errors
    ///
    /// Returns `VersionError` if version calculation fails.
    pub fn plan_releases(
        changesets: &[Changeset],
        packages: &[PackageInfo],
    ) -> Result<ReleasePlan, VersionError> {
        Self::plan_releases_with_prerelease(changesets, packages, None)
    }

    /// Plans version releases with optional prerelease specification.
    ///
    /// # Errors
    ///
    /// Returns `VersionError` if version calculation fails.
    pub fn plan_releases_with_prerelease(
        changesets: &[Changeset],
        packages: &[PackageInfo],
        prerelease: Option<&PrereleaseSpec>,
    ) -> Result<ReleasePlan, VersionError> {
        let package_lookup: IndexMap<_, _> = packages.iter().map(|p| (p.name.clone(), p)).collect();
        let bumps_by_package = Self::aggregate_bumps(changesets);

        let mut releases = Vec::new();
        let mut unknown_packages = Vec::new();

        for (name, bumps) in &bumps_by_package {
            let bump_type = max_bump_type(bumps);

            if bump_type.is_none() && prerelease.is_none() {
                continue;
            }

            if let Some(pkg) = package_lookup.get(name) {
                let new_version = calculate_new_version(&pkg.version, bump_type, prerelease)?;
                let effective_bump = bump_type.unwrap_or(BumpType::Patch);
                releases.push(PackageVersion {
                    name: name.clone(),
                    current_version: pkg.version.clone(),
                    new_version,
                    bump_type: effective_bump,
                });
            } else {
                unknown_packages.push(name.clone());
            }
        }

        Ok(ReleasePlan {
            releases,
            unknown_packages,
        })
    }

    /// Plans graduation of prerelease versions to stable.
    ///
    /// # Errors
    ///
    /// Returns `VersionError` if version calculation fails.
    pub fn plan_graduation(packages: &[PackageInfo]) -> Result<ReleasePlan, VersionError> {
        let mut releases = Vec::new();

        for pkg in packages {
            if changeset_version::is_prerelease(&pkg.version) {
                let new_version = calculate_new_version(&pkg.version, None, None)?;
                releases.push(PackageVersion {
                    name: pkg.name.clone(),
                    current_version: pkg.version.clone(),
                    new_version,
                    bump_type: BumpType::Patch,
                });
            }
        }

        Ok(ReleasePlan {
            releases,
            unknown_packages: Vec::new(),
        })
    }

    /// Plans version releases with special handling for 0.x versions.
    ///
    /// # Errors
    ///
    /// Returns `VersionError` if version calculation fails.
    pub fn plan_releases_with_behavior(
        changesets: &[Changeset],
        packages: &[PackageInfo],
        prerelease: Option<&PrereleaseSpec>,
        zero_behavior: ZeroVersionBehavior,
    ) -> Result<ReleasePlan, VersionError> {
        let package_lookup: IndexMap<_, _> = packages.iter().map(|p| (p.name.clone(), p)).collect();
        let bumps_by_package = Self::aggregate_bumps(changesets);
        let graduates = Self::collect_graduates(changesets);

        let mut releases = Vec::new();
        let mut unknown_packages = Vec::new();

        for (name, bumps) in &bumps_by_package {
            let bump_type = max_bump_type(bumps);
            let should_graduate = graduates.contains(name);

            if bump_type.is_none() && prerelease.is_none() && !should_graduate {
                continue;
            }

            if let Some(pkg) = package_lookup.get(name) {
                let new_version = calculate_new_version_with_zero_behavior(
                    &pkg.version,
                    bump_type,
                    prerelease,
                    zero_behavior,
                    should_graduate,
                )?;
                let effective_bump = bump_type.unwrap_or(BumpType::Patch);
                releases.push(PackageVersion {
                    name: name.clone(),
                    current_version: pkg.version.clone(),
                    new_version,
                    bump_type: effective_bump,
                });
            } else {
                unknown_packages.push(name.clone());
            }
        }

        Ok(ReleasePlan {
            releases,
            unknown_packages,
        })
    }

    /// Plans graduation of 0.x versions to 1.0.0.
    ///
    /// # Errors
    ///
    /// Returns `VersionError` if version calculation fails.
    pub fn plan_zero_graduation(
        packages: &[PackageInfo],
        prerelease: Option<&PrereleaseSpec>,
    ) -> Result<ReleasePlan, VersionError> {
        let mut releases = Vec::new();

        for pkg in packages {
            if is_zero_version(&pkg.version) {
                let new_version = calculate_new_version_with_zero_behavior(
                    &pkg.version,
                    None,
                    prerelease,
                    ZeroVersionBehavior::EffectiveMinor,
                    true,
                )?;
                releases.push(PackageVersion {
                    name: pkg.name.clone(),
                    current_version: pkg.version.clone(),
                    new_version,
                    bump_type: BumpType::Major,
                });
            }
        }

        Ok(ReleasePlan {
            releases,
            unknown_packages: Vec::new(),
        })
    }

    /// Plans version releases with per-package configuration.
    ///
    /// This method applies individual prerelease tags and graduation settings
    /// to each package based on the validated configuration from CLI + TOML.
    ///
    /// # Errors
    ///
    /// Returns `VersionError` if version calculation fails.
    pub fn plan_releases_per_package(
        changesets: &[Changeset],
        packages: &[PackageInfo],
        per_package_config: &HashMap<String, PackageReleaseConfig>,
        zero_behavior: ZeroVersionBehavior,
    ) -> Result<ReleasePlan, VersionError> {
        let package_lookup: IndexMap<_, _> = packages.iter().map(|p| (p.name.clone(), p)).collect();
        let bumps_by_package = Self::aggregate_bumps(changesets);
        let changeset_graduates = Self::collect_graduates(changesets);

        let mut releases = Vec::new();
        let mut unknown_packages = Vec::new();

        for (name, bumps) in &bumps_by_package {
            let bump_type = max_bump_type(bumps);
            let config = per_package_config.get(name);

            let prerelease = config.and_then(|c| c.prerelease.as_ref());
            let should_graduate =
                config.is_some_and(|c| c.graduate_zero) || changeset_graduates.contains(name);

            if bump_type.is_none() && prerelease.is_none() && !should_graduate {
                continue;
            }

            if let Some(pkg) = package_lookup.get(name) {
                let new_version = calculate_new_version_with_zero_behavior(
                    &pkg.version,
                    bump_type,
                    prerelease,
                    zero_behavior,
                    should_graduate,
                )?;
                let effective_bump = bump_type.unwrap_or(BumpType::Patch);
                releases.push(PackageVersion {
                    name: name.clone(),
                    current_version: pkg.version.clone(),
                    new_version,
                    bump_type: effective_bump,
                });
            } else {
                unknown_packages.push(name.clone());
            }
        }

        for (name, config) in per_package_config {
            if bumps_by_package.contains_key(name) {
                continue;
            }

            if config.prerelease.is_none() && !config.graduate_zero {
                continue;
            }

            if let Some(pkg) = package_lookup.get(name) {
                let new_version = calculate_new_version_with_zero_behavior(
                    &pkg.version,
                    None,
                    config.prerelease.as_ref(),
                    zero_behavior,
                    config.graduate_zero,
                )?;
                releases.push(PackageVersion {
                    name: name.clone(),
                    current_version: pkg.version.clone(),
                    new_version,
                    bump_type: BumpType::Major,
                });
            }
        }

        Ok(ReleasePlan {
            releases,
            unknown_packages,
        })
    }

    fn collect_graduates(changesets: &[Changeset]) -> HashSet<String> {
        changesets
            .iter()
            .filter(|c| c.graduate)
            .flat_map(|c| c.releases.iter().map(|r| r.name.clone()))
            .collect()
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
            consumed_for_prerelease: None,
            graduate: false,
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
            consumed_for_prerelease: None,
            graduate: false,
        }
    }

    #[test]
    fn plan_releases_empty_changesets_returns_empty_plan() {
        let packages = vec![make_package("my-crate", "1.0.0")];

        let plan = VersionPlanner::plan_releases(&[], &packages).expect("plan_releases");

        assert!(plan.releases.is_empty());
        assert!(plan.unknown_packages.is_empty());
    }

    #[test]
    fn plan_releases_single_package_single_bump() {
        let packages = vec![make_package("my-crate", "1.0.0")];
        let changesets = vec![make_changeset("my-crate", BumpType::Patch, "Fix bug")];

        let plan = VersionPlanner::plan_releases(&changesets, &packages).expect("plan_releases");

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

        let plan = VersionPlanner::plan_releases(&changesets, &packages).expect("plan_releases");

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

        let plan = VersionPlanner::plan_releases(&changesets, &packages).expect("plan_releases");

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

        let plan = VersionPlanner::plan_releases(&changesets, &packages).expect("plan_releases");

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

        let plan = VersionPlanner::plan_releases(&changesets, &packages).expect("plan_releases");

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

        let plan = VersionPlanner::plan_releases(&changesets, &packages).expect("plan_releases");

        assert_eq!(plan.releases.len(), 1);
        let release = &plan.releases[0];
        assert_eq!(
            release.current_version,
            "1.0.0-alpha.1".parse::<Version>().expect("valid")
        );
        assert!(release.new_version > release.current_version);
    }

    #[test]
    fn plan_releases_handles_build_metadata() {
        let packages = vec![make_package("my-crate", "1.0.0+build.123")];
        let changesets = vec![make_changeset("my-crate", BumpType::Minor, "Feature")];

        let plan = VersionPlanner::plan_releases(&changesets, &packages).expect("plan_releases");

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

        let plan = VersionPlanner::plan_releases(&changesets, &packages).expect("plan_releases");

        assert_eq!(plan.releases.len(), 1);
        let release = &plan.releases[0];
        assert_eq!(release.current_version, Version::new(0, 1, 0));
        assert_eq!(release.new_version, Version::new(1, 0, 0));
    }

    #[test]
    fn plan_releases_with_prerelease_creates_alpha_version() {
        let packages = vec![make_package("my-crate", "1.0.0")];
        let changesets = vec![make_changeset("my-crate", BumpType::Patch, "Fix")];

        let plan = VersionPlanner::plan_releases_with_prerelease(
            &changesets,
            &packages,
            Some(&PrereleaseSpec::Alpha),
        )
        .expect("plan_releases_with_prerelease");

        assert_eq!(plan.releases.len(), 1);
        let release = &plan.releases[0];
        assert_eq!(
            release.new_version,
            "1.0.1-alpha.1".parse::<Version>().expect("valid")
        );
    }

    #[test]
    fn plan_releases_with_prerelease_increments_existing() {
        let packages = vec![make_package("my-crate", "1.0.1-alpha.2")];
        let changesets = vec![make_changeset("my-crate", BumpType::Patch, "Fix")];

        let plan = VersionPlanner::plan_releases_with_prerelease(
            &changesets,
            &packages,
            Some(&PrereleaseSpec::Alpha),
        )
        .expect("plan_releases_with_prerelease");

        assert_eq!(plan.releases.len(), 1);
        let release = &plan.releases[0];
        assert_eq!(
            release.new_version,
            "1.0.1-alpha.3".parse::<Version>().expect("valid")
        );
    }

    #[test]
    fn plan_graduation_creates_stable_from_prerelease() {
        let packages = vec![
            make_package("crate-a", "1.0.1-rc.1"),
            make_package("crate-b", "2.0.0"),
        ];

        let plan = VersionPlanner::plan_graduation(&packages).expect("plan_graduation");

        assert_eq!(plan.releases.len(), 1);
        let release = &plan.releases[0];
        assert_eq!(release.name, "crate-a");
        assert_eq!(release.new_version, Version::new(1, 0, 1));
    }

    #[test]
    fn plan_graduation_empty_for_all_stable() {
        let packages = vec![
            make_package("crate-a", "1.0.0"),
            make_package("crate-b", "2.0.0"),
        ];

        let plan = VersionPlanner::plan_graduation(&packages).expect("plan_graduation");

        assert!(plan.releases.is_empty());
    }

    mod zero_version_behavior_tests {
        use super::*;

        #[test]
        fn effective_minor_converts_major_to_minor() {
            let packages = vec![make_package("my-crate", "0.1.2")];
            let changesets = vec![make_changeset("my-crate", BumpType::Major, "Breaking")];

            let plan = VersionPlanner::plan_releases_with_behavior(
                &changesets,
                &packages,
                None,
                ZeroVersionBehavior::EffectiveMinor,
            )
            .expect("plan_releases_with_behavior");

            assert_eq!(plan.releases.len(), 1);
            let release = &plan.releases[0];
            assert_eq!(release.new_version, Version::new(0, 2, 0));
        }

        #[test]
        fn effective_minor_converts_minor_to_patch() {
            let packages = vec![make_package("my-crate", "0.1.2")];
            let changesets = vec![make_changeset("my-crate", BumpType::Minor, "Feature")];

            let plan = VersionPlanner::plan_releases_with_behavior(
                &changesets,
                &packages,
                None,
                ZeroVersionBehavior::EffectiveMinor,
            )
            .expect("plan_releases_with_behavior");

            assert_eq!(plan.releases.len(), 1);
            let release = &plan.releases[0];
            assert_eq!(release.new_version, Version::new(0, 1, 3));
        }

        #[test]
        fn auto_promote_major_becomes_1_0_0() {
            let packages = vec![make_package("my-crate", "0.1.2")];
            let changesets = vec![make_changeset("my-crate", BumpType::Major, "Breaking")];

            let plan = VersionPlanner::plan_releases_with_behavior(
                &changesets,
                &packages,
                None,
                ZeroVersionBehavior::AutoPromoteOnMajor,
            )
            .expect("plan_releases_with_behavior");

            assert_eq!(plan.releases.len(), 1);
            let release = &plan.releases[0];
            assert_eq!(release.new_version, Version::new(1, 0, 0));
        }

        #[test]
        fn auto_promote_minor_stays_minor() {
            let packages = vec![make_package("my-crate", "0.1.2")];
            let changesets = vec![make_changeset("my-crate", BumpType::Minor, "Feature")];

            let plan = VersionPlanner::plan_releases_with_behavior(
                &changesets,
                &packages,
                None,
                ZeroVersionBehavior::AutoPromoteOnMajor,
            )
            .expect("plan_releases_with_behavior");

            assert_eq!(plan.releases.len(), 1);
            let release = &plan.releases[0];
            assert_eq!(release.new_version, Version::new(0, 2, 0));
        }

        #[test]
        fn stable_version_unaffected_by_behavior() {
            let packages = vec![make_package("my-crate", "1.2.3")];
            let changesets = vec![make_changeset("my-crate", BumpType::Major, "Breaking")];

            let plan = VersionPlanner::plan_releases_with_behavior(
                &changesets,
                &packages,
                None,
                ZeroVersionBehavior::EffectiveMinor,
            )
            .expect("plan_releases_with_behavior");

            assert_eq!(plan.releases.len(), 1);
            let release = &plan.releases[0];
            assert_eq!(release.new_version, Version::new(2, 0, 0));
        }

        #[test]
        fn with_prerelease_tag() {
            let packages = vec![make_package("my-crate", "0.1.2")];
            let changesets = vec![make_changeset("my-crate", BumpType::Major, "Breaking")];

            let plan = VersionPlanner::plan_releases_with_behavior(
                &changesets,
                &packages,
                Some(&PrereleaseSpec::Alpha),
                ZeroVersionBehavior::EffectiveMinor,
            )
            .expect("plan_releases_with_behavior");

            assert_eq!(plan.releases.len(), 1);
            let release = &plan.releases[0];
            assert_eq!(
                release.new_version,
                "0.2.0-alpha.1".parse::<Version>().expect("valid")
            );
        }
    }

    mod zero_graduation_tests {
        use super::*;

        #[test]
        fn graduates_zero_version_to_1_0_0() {
            let packages = vec![
                make_package("crate-a", "0.5.3"),
                make_package("crate-b", "1.0.0"),
            ];

            let plan = VersionPlanner::plan_zero_graduation(&packages, None)
                .expect("plan_zero_graduation");

            assert_eq!(plan.releases.len(), 1);
            let release = &plan.releases[0];
            assert_eq!(release.name, "crate-a");
            assert_eq!(release.new_version, Version::new(1, 0, 0));
        }

        #[test]
        fn graduates_with_prerelease() {
            let packages = vec![make_package("crate-a", "0.5.3")];

            let plan =
                VersionPlanner::plan_zero_graduation(&packages, Some(&PrereleaseSpec::Alpha))
                    .expect("plan_zero_graduation");

            assert_eq!(plan.releases.len(), 1);
            let release = &plan.releases[0];
            assert_eq!(
                release.new_version,
                "1.0.0-alpha.1".parse::<Version>().expect("valid")
            );
        }

        #[test]
        fn empty_for_all_stable() {
            let packages = vec![
                make_package("crate-a", "1.0.0"),
                make_package("crate-b", "2.5.0"),
            ];

            let plan = VersionPlanner::plan_zero_graduation(&packages, None)
                .expect("plan_zero_graduation");

            assert!(plan.releases.is_empty());
        }

        #[test]
        fn multiple_zero_versions() {
            let packages = vec![
                make_package("crate-a", "0.1.0"),
                make_package("crate-b", "0.5.3"),
                make_package("crate-c", "1.0.0"),
            ];

            let plan = VersionPlanner::plan_zero_graduation(&packages, None)
                .expect("plan_zero_graduation");

            assert_eq!(plan.releases.len(), 2);
            for release in &plan.releases {
                assert_eq!(release.new_version, Version::new(1, 0, 0));
            }
        }
    }

    mod changeset_graduate_field_tests {
        use super::*;

        fn make_graduating_changeset(package_name: &str, bump: BumpType) -> Changeset {
            Changeset {
                summary: "Graduate to 1.0".to_string(),
                releases: vec![PackageRelease {
                    name: package_name.to_string(),
                    bump_type: bump,
                }],
                category: ChangeCategory::Changed,
                consumed_for_prerelease: None,
                graduate: true,
            }
        }

        #[test]
        fn graduate_field_triggers_graduation() {
            let packages = vec![make_package("my-crate", "0.5.3")];
            let changesets = vec![make_graduating_changeset("my-crate", BumpType::Major)];

            let plan = VersionPlanner::plan_releases_with_behavior(
                &changesets,
                &packages,
                None,
                ZeroVersionBehavior::EffectiveMinor,
            )
            .expect("plan_releases_with_behavior");

            assert_eq!(plan.releases.len(), 1);
            let release = &plan.releases[0];
            assert_eq!(release.new_version, Version::new(1, 0, 0));
        }

        #[test]
        fn graduate_field_ignored_for_stable_returns_error() {
            let packages = vec![make_package("my-crate", "1.2.3")];
            let changesets = vec![make_graduating_changeset("my-crate", BumpType::Major)];

            let result = VersionPlanner::plan_releases_with_behavior(
                &changesets,
                &packages,
                None,
                ZeroVersionBehavior::EffectiveMinor,
            );

            assert!(result.is_err());
        }

        #[test]
        fn mixed_graduate_and_regular_changesets() {
            let packages = vec![
                make_package("graduating", "0.5.0"),
                make_package("regular", "0.3.0"),
            ];
            let changesets = vec![
                make_graduating_changeset("graduating", BumpType::Major),
                make_changeset("regular", BumpType::Major, "Breaking"),
            ];

            let plan = VersionPlanner::plan_releases_with_behavior(
                &changesets,
                &packages,
                None,
                ZeroVersionBehavior::EffectiveMinor,
            )
            .expect("plan_releases_with_behavior");

            assert_eq!(plan.releases.len(), 2);

            let graduating = plan
                .releases
                .iter()
                .find(|r| r.name == "graduating")
                .expect("graduating should be in releases");
            assert_eq!(graduating.new_version, Version::new(1, 0, 0));

            let regular = plan
                .releases
                .iter()
                .find(|r| r.name == "regular")
                .expect("regular should be in releases");
            assert_eq!(regular.new_version, Version::new(0, 4, 0));
        }
    }

    mod per_package_config_tests {
        use super::*;

        #[test]
        fn per_package_prerelease_applies_to_specific_crate() {
            let packages = vec![
                make_package("crate-a", "1.0.0"),
                make_package("crate-b", "1.0.0"),
            ];
            let changesets = vec![
                make_changeset("crate-a", BumpType::Patch, "Fix A"),
                make_changeset("crate-b", BumpType::Patch, "Fix B"),
            ];

            let mut config = HashMap::new();
            config.insert(
                "crate-a".to_string(),
                PackageReleaseConfig {
                    prerelease: Some(PrereleaseSpec::Alpha),
                    graduate_zero: false,
                },
            );

            let plan = VersionPlanner::plan_releases_per_package(
                &changesets,
                &packages,
                &config,
                ZeroVersionBehavior::EffectiveMinor,
            )
            .expect("plan_releases_per_package");

            let release_a = plan
                .releases
                .iter()
                .find(|r| r.name == "crate-a")
                .expect("crate-a should be in releases");
            let release_b = plan
                .releases
                .iter()
                .find(|r| r.name == "crate-b")
                .expect("crate-b should be in releases");

            assert_eq!(
                release_a.new_version,
                "1.0.1-alpha.1".parse::<Version>().expect("valid")
            );
            assert_eq!(release_b.new_version, Version::new(1, 0, 1));
        }

        #[test]
        fn per_package_graduation_applies_to_specific_crate() {
            let packages = vec![
                make_package("crate-a", "0.5.0"),
                make_package("crate-b", "0.3.0"),
            ];
            let changesets = vec![
                make_changeset("crate-a", BumpType::Minor, "Feature A"),
                make_changeset("crate-b", BumpType::Minor, "Feature B"),
            ];

            let mut config = HashMap::new();
            config.insert(
                "crate-a".to_string(),
                PackageReleaseConfig {
                    prerelease: None,
                    graduate_zero: true,
                },
            );

            let plan = VersionPlanner::plan_releases_per_package(
                &changesets,
                &packages,
                &config,
                ZeroVersionBehavior::EffectiveMinor,
            )
            .expect("plan_releases_per_package");

            let release_a = plan
                .releases
                .iter()
                .find(|r| r.name == "crate-a")
                .expect("crate-a should be in releases");
            let release_b = plan
                .releases
                .iter()
                .find(|r| r.name == "crate-b")
                .expect("crate-b should be in releases");

            assert_eq!(release_a.new_version, Version::new(1, 0, 0));
            assert_eq!(release_b.new_version, Version::new(0, 3, 1));
        }

        #[test]
        fn graduation_without_changesets() {
            let packages = vec![make_package("crate-a", "0.5.0")];
            let changesets: Vec<Changeset> = vec![];

            let mut config = HashMap::new();
            config.insert(
                "crate-a".to_string(),
                PackageReleaseConfig {
                    prerelease: None,
                    graduate_zero: true,
                },
            );

            let plan = VersionPlanner::plan_releases_per_package(
                &changesets,
                &packages,
                &config,
                ZeroVersionBehavior::EffectiveMinor,
            )
            .expect("plan_releases_per_package");

            assert_eq!(plan.releases.len(), 1);
            assert_eq!(plan.releases[0].new_version, Version::new(1, 0, 0));
        }

        #[test]
        fn graduation_with_prerelease_creates_prerelease_1_0_0() {
            let packages = vec![make_package("crate-a", "0.5.0")];
            let changesets = vec![make_changeset("crate-a", BumpType::Minor, "Feature")];

            let mut config = HashMap::new();
            config.insert(
                "crate-a".to_string(),
                PackageReleaseConfig {
                    prerelease: Some(PrereleaseSpec::Rc),
                    graduate_zero: true,
                },
            );

            let plan = VersionPlanner::plan_releases_per_package(
                &changesets,
                &packages,
                &config,
                ZeroVersionBehavior::EffectiveMinor,
            )
            .expect("plan_releases_per_package");

            assert_eq!(plan.releases.len(), 1);
            assert_eq!(
                plan.releases[0].new_version,
                "1.0.0-rc.1".parse::<Version>().expect("valid")
            );
        }

        #[test]
        fn empty_config_uses_defaults() {
            let packages = vec![make_package("crate-a", "1.0.0")];
            let changesets = vec![make_changeset("crate-a", BumpType::Patch, "Fix")];
            let config = HashMap::new();

            let plan = VersionPlanner::plan_releases_per_package(
                &changesets,
                &packages,
                &config,
                ZeroVersionBehavior::EffectiveMinor,
            )
            .expect("plan_releases_per_package");

            assert_eq!(plan.releases.len(), 1);
            assert_eq!(plan.releases[0].new_version, Version::new(1, 0, 1));
        }

        #[test]
        fn mixed_prerelease_and_stable_releases() {
            let packages = vec![
                make_package("alpha-crate", "1.0.0"),
                make_package("beta-crate", "2.0.0"),
                make_package("stable-crate", "3.0.0"),
            ];
            let changesets = vec![
                make_changeset("alpha-crate", BumpType::Minor, "Feature"),
                make_changeset("beta-crate", BumpType::Patch, "Fix"),
                make_changeset("stable-crate", BumpType::Major, "Breaking"),
            ];

            let mut config = HashMap::new();
            config.insert(
                "alpha-crate".to_string(),
                PackageReleaseConfig {
                    prerelease: Some(PrereleaseSpec::Alpha),
                    graduate_zero: false,
                },
            );
            config.insert(
                "beta-crate".to_string(),
                PackageReleaseConfig {
                    prerelease: Some(PrereleaseSpec::Beta),
                    graduate_zero: false,
                },
            );

            let plan = VersionPlanner::plan_releases_per_package(
                &changesets,
                &packages,
                &config,
                ZeroVersionBehavior::EffectiveMinor,
            )
            .expect("plan_releases_per_package");

            assert_eq!(plan.releases.len(), 3);

            let alpha = plan
                .releases
                .iter()
                .find(|r| r.name == "alpha-crate")
                .expect("alpha-crate should be in releases");
            let beta = plan
                .releases
                .iter()
                .find(|r| r.name == "beta-crate")
                .expect("beta-crate should be in releases");
            let stable = plan
                .releases
                .iter()
                .find(|r| r.name == "stable-crate")
                .expect("stable-crate should be in releases");

            assert_eq!(
                alpha.new_version,
                "1.1.0-alpha.1".parse::<Version>().expect("valid")
            );
            assert_eq!(
                beta.new_version,
                "2.0.1-beta.1".parse::<Version>().expect("valid")
            );
            assert_eq!(stable.new_version, Version::new(4, 0, 0));
        }

        #[test]
        fn changeset_graduate_field_combined_with_config() {
            let packages = vec![make_package("crate-a", "0.5.0")];

            let changesets = vec![Changeset {
                summary: "Graduate".to_string(),
                releases: vec![PackageRelease {
                    name: "crate-a".to_string(),
                    bump_type: BumpType::Major,
                }],
                category: ChangeCategory::Changed,
                consumed_for_prerelease: None,
                graduate: true,
            }];

            let mut config = HashMap::new();
            config.insert(
                "crate-a".to_string(),
                PackageReleaseConfig {
                    prerelease: Some(PrereleaseSpec::Rc),
                    graduate_zero: false,
                },
            );

            let plan = VersionPlanner::plan_releases_per_package(
                &changesets,
                &packages,
                &config,
                ZeroVersionBehavior::EffectiveMinor,
            )
            .expect("plan_releases_per_package");

            assert_eq!(plan.releases.len(), 1);
            assert_eq!(
                plan.releases[0].new_version,
                "1.0.0-rc.1".parse::<Version>().expect("valid")
            );
        }

        #[test]
        fn config_graduation_without_changeset_graduation() {
            let packages = vec![make_package("crate-a", "0.5.0")];
            let changesets = vec![make_changeset("crate-a", BumpType::Minor, "Feature")];

            let mut config = HashMap::new();
            config.insert(
                "crate-a".to_string(),
                PackageReleaseConfig {
                    prerelease: None,
                    graduate_zero: true,
                },
            );

            let plan = VersionPlanner::plan_releases_per_package(
                &changesets,
                &packages,
                &config,
                ZeroVersionBehavior::EffectiveMinor,
            )
            .expect("plan_releases_per_package");

            assert_eq!(plan.releases.len(), 1);
            assert_eq!(plan.releases[0].new_version, Version::new(1, 0, 0));
        }

        #[test]
        fn unknown_package_in_changeset_collected() {
            let packages = vec![make_package("known", "1.0.0")];
            let changesets = vec![make_changeset("unknown", BumpType::Patch, "Fix")];
            let config = HashMap::new();

            let plan = VersionPlanner::plan_releases_per_package(
                &changesets,
                &packages,
                &config,
                ZeroVersionBehavior::EffectiveMinor,
            )
            .expect("plan_releases_per_package");

            assert!(plan.releases.is_empty());
            assert_eq!(plan.unknown_packages, vec!["unknown"]);
        }

        #[test]
        fn config_without_changesets_ignored_for_unknown_package() {
            let packages = vec![make_package("known", "1.0.0")];
            let changesets: Vec<Changeset> = vec![];

            let mut config = HashMap::new();
            config.insert(
                "unknown".to_string(),
                PackageReleaseConfig {
                    prerelease: Some(PrereleaseSpec::Alpha),
                    graduate_zero: false,
                },
            );

            let plan = VersionPlanner::plan_releases_per_package(
                &changesets,
                &packages,
                &config,
                ZeroVersionBehavior::EffectiveMinor,
            )
            .expect("plan_releases_per_package");

            assert!(plan.releases.is_empty());
        }

        #[test]
        fn prerelease_only_config_without_changesets() {
            let packages = vec![make_package("crate-a", "1.0.0")];
            let changesets: Vec<Changeset> = vec![];

            let mut config = HashMap::new();
            config.insert(
                "crate-a".to_string(),
                PackageReleaseConfig {
                    prerelease: Some(PrereleaseSpec::Alpha),
                    graduate_zero: false,
                },
            );

            let plan = VersionPlanner::plan_releases_per_package(
                &changesets,
                &packages,
                &config,
                ZeroVersionBehavior::EffectiveMinor,
            )
            .expect("plan_releases_per_package");

            assert_eq!(plan.releases.len(), 1);
            assert_eq!(
                plan.releases[0].new_version,
                "1.0.1-alpha.1".parse::<Version>().expect("valid")
            );
        }

        #[test]
        fn zero_behavior_applied_with_config() {
            let packages = vec![make_package("crate-a", "0.5.0")];
            let changesets = vec![make_changeset("crate-a", BumpType::Major, "Breaking")];
            let config = HashMap::new();

            let plan = VersionPlanner::plan_releases_per_package(
                &changesets,
                &packages,
                &config,
                ZeroVersionBehavior::EffectiveMinor,
            )
            .expect("plan_releases_per_package");

            assert_eq!(plan.releases.len(), 1);
            assert_eq!(plan.releases[0].new_version, Version::new(0, 6, 0));
        }

        #[test]
        fn auto_promote_behavior_with_config() {
            let packages = vec![make_package("crate-a", "0.5.0")];
            let changesets = vec![make_changeset("crate-a", BumpType::Major, "Breaking")];
            let config = HashMap::new();

            let plan = VersionPlanner::plan_releases_per_package(
                &changesets,
                &packages,
                &config,
                ZeroVersionBehavior::AutoPromoteOnMajor,
            )
            .expect("plan_releases_per_package");

            assert_eq!(plan.releases.len(), 1);
            assert_eq!(plan.releases[0].new_version, Version::new(1, 0, 0));
        }

        #[test]
        fn package_with_no_changesets_and_no_config_not_included() {
            let packages = vec![
                make_package("with-changeset", "1.0.0"),
                make_package("no-changeset", "2.0.0"),
            ];
            let changesets = vec![make_changeset("with-changeset", BumpType::Patch, "Fix")];
            let config = HashMap::new();

            let plan = VersionPlanner::plan_releases_per_package(
                &changesets,
                &packages,
                &config,
                ZeroVersionBehavior::EffectiveMinor,
            )
            .expect("plan_releases_per_package");

            assert_eq!(plan.releases.len(), 1);
            assert_eq!(plan.releases[0].name, "with-changeset");
            assert!(plan.unknown_packages.is_empty());
        }

        #[test]
        fn graduation_only_config_without_changeset() {
            let packages = vec![make_package("crate-a", "0.5.0")];
            let changesets: Vec<Changeset> = vec![];

            let mut config = HashMap::new();
            config.insert(
                "crate-a".to_string(),
                PackageReleaseConfig {
                    prerelease: None,
                    graduate_zero: true,
                },
            );

            let plan = VersionPlanner::plan_releases_per_package(
                &changesets,
                &packages,
                &config,
                ZeroVersionBehavior::EffectiveMinor,
            )
            .expect("plan_releases_per_package");

            assert_eq!(plan.releases.len(), 1);
            assert_eq!(plan.releases[0].new_version, Version::new(1, 0, 0));
        }

        #[test]
        fn config_with_neither_prerelease_nor_graduation_not_included() {
            let packages = vec![make_package("crate-a", "1.0.0")];
            let changesets: Vec<Changeset> = vec![];

            let mut config = HashMap::new();
            config.insert(
                "crate-a".to_string(),
                PackageReleaseConfig {
                    prerelease: None,
                    graduate_zero: false,
                },
            );

            let plan = VersionPlanner::plan_releases_per_package(
                &changesets,
                &packages,
                &config,
                ZeroVersionBehavior::EffectiveMinor,
            )
            .expect("plan_releases_per_package");

            assert!(plan.releases.is_empty());
        }

        #[test]
        fn all_packages_graduating_simultaneously() {
            let packages = vec![
                make_package("crate-a", "0.1.0"),
                make_package("crate-b", "0.5.0"),
                make_package("crate-c", "0.9.0"),
            ];
            let changesets: Vec<Changeset> = vec![];

            let mut config = HashMap::new();
            for pkg in &packages {
                config.insert(
                    pkg.name.clone(),
                    PackageReleaseConfig {
                        prerelease: None,
                        graduate_zero: true,
                    },
                );
            }

            let plan = VersionPlanner::plan_releases_per_package(
                &changesets,
                &packages,
                &config,
                ZeroVersionBehavior::EffectiveMinor,
            )
            .expect("plan_releases_per_package");

            assert_eq!(plan.releases.len(), 3);
            for release in &plan.releases {
                assert_eq!(
                    release.new_version,
                    Version::new(1, 0, 0),
                    "{} should graduate to 1.0.0",
                    release.name
                );
            }
        }

        #[test]
        fn prerelease_graduation_with_prerelease_tag() {
            let packages = vec![make_package("crate-a", "0.5.0")];
            let changesets: Vec<Changeset> = vec![];

            let mut config = HashMap::new();
            config.insert(
                "crate-a".to_string(),
                PackageReleaseConfig {
                    prerelease: Some(PrereleaseSpec::Beta),
                    graduate_zero: true,
                },
            );

            let plan = VersionPlanner::plan_releases_per_package(
                &changesets,
                &packages,
                &config,
                ZeroVersionBehavior::EffectiveMinor,
            )
            .expect("plan_releases_per_package");

            assert_eq!(plan.releases.len(), 1);
            assert_eq!(
                plan.releases[0].new_version,
                "1.0.0-beta.1".parse::<Version>().expect("valid")
            );
        }
    }

    mod auto_promote_zero_behavior {
        use super::*;

        #[test]
        fn auto_promote_with_major_bump_graduates() {
            let packages = vec![make_package("my-crate", "0.1.2")];
            let changesets = vec![make_changeset("my-crate", BumpType::Major, "Breaking")];

            let plan = VersionPlanner::plan_releases_with_behavior(
                &changesets,
                &packages,
                None,
                ZeroVersionBehavior::AutoPromoteOnMajor,
            )
            .expect("plan_releases_with_behavior");

            assert_eq!(plan.releases.len(), 1);
            assert_eq!(plan.releases[0].new_version, Version::new(1, 0, 0));
        }

        #[test]
        fn auto_promote_with_minor_bump_stays_zero() {
            let packages = vec![make_package("my-crate", "0.1.2")];
            let changesets = vec![make_changeset("my-crate", BumpType::Minor, "Feature")];

            let plan = VersionPlanner::plan_releases_with_behavior(
                &changesets,
                &packages,
                None,
                ZeroVersionBehavior::AutoPromoteOnMajor,
            )
            .expect("plan_releases_with_behavior");

            assert_eq!(plan.releases.len(), 1);
            assert_eq!(plan.releases[0].new_version, Version::new(0, 2, 0));
        }

        #[test]
        fn auto_promote_with_patch_bump_stays_zero() {
            let packages = vec![make_package("my-crate", "0.1.2")];
            let changesets = vec![make_changeset("my-crate", BumpType::Patch, "Fix")];

            let plan = VersionPlanner::plan_releases_with_behavior(
                &changesets,
                &packages,
                None,
                ZeroVersionBehavior::AutoPromoteOnMajor,
            )
            .expect("plan_releases_with_behavior");

            assert_eq!(plan.releases.len(), 1);
            assert_eq!(plan.releases[0].new_version, Version::new(0, 1, 3));
        }
    }

    mod unknown_packages {
        use super::*;

        #[test]
        fn multiple_unknown_packages_collected() {
            let packages = vec![make_package("known", "1.0.0")];
            let changesets = vec![make_multi_changeset(
                vec![
                    ("unknown1", BumpType::Patch),
                    ("unknown2", BumpType::Minor),
                    ("unknown3", BumpType::Major),
                ],
                "Changes to unknown packages",
            )];

            let plan =
                VersionPlanner::plan_releases(&changesets, &packages).expect("plan_releases");

            assert!(plan.releases.is_empty());
            assert_eq!(plan.unknown_packages.len(), 3);
            assert!(plan.unknown_packages.contains(&"unknown1".to_string()));
            assert!(plan.unknown_packages.contains(&"unknown2".to_string()));
            assert!(plan.unknown_packages.contains(&"unknown3".to_string()));
        }

        #[test]
        fn per_package_config_for_nonexistent_is_silently_ignored() {
            let packages = vec![make_package("known", "1.0.0")];
            let changesets: Vec<Changeset> = vec![];

            let mut config = HashMap::new();
            config.insert(
                "nonexistent".to_string(),
                PackageReleaseConfig {
                    prerelease: Some(PrereleaseSpec::Alpha),
                    graduate_zero: false,
                },
            );

            let plan = VersionPlanner::plan_releases_per_package(
                &changesets,
                &packages,
                &config,
                ZeroVersionBehavior::EffectiveMinor,
            )
            .expect("plan_releases_per_package");

            assert!(plan.releases.is_empty());
            assert!(
                plan.unknown_packages.is_empty(),
                "config for nonexistent packages does not add to unknown_packages"
            );
        }
    }
}
