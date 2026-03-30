use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use changeset_core::PackageInfo;

use super::types::ReleaseInput;
use crate::types::{PackageReleaseConfig, PackageVersion};

pub(super) enum EarlyReturnDecision {
    Continue,
    NoChangesets,
    ForceRequired,
}

pub(super) fn is_any_prerelease_configured(
    input: &ReleaseInput,
    per_package_config: &HashMap<String, PackageReleaseConfig>,
) -> bool {
    input.global_prerelease().is_some()
        || per_package_config
            .values()
            .any(|c| c.prerelease().is_some())
}

pub(super) fn is_prerelease_graduation(
    packages: &[PackageInfo],
    per_package_config: &HashMap<String, PackageReleaseConfig>,
) -> bool {
    if per_package_config
        .values()
        .any(|c| c.prerelease().is_some())
    {
        return false;
    }
    packages
        .iter()
        .any(|p| changeset_version::is_prerelease(p.version()))
}

pub(super) fn is_zero_graduation(
    packages: &[PackageInfo],
    input: &ReleaseInput,
    per_package_config: &HashMap<String, PackageReleaseConfig>,
) -> bool {
    let has_graduation = input.graduate_all()
        || per_package_config
            .values()
            .any(PackageReleaseConfig::graduate_zero);
    if !has_graduation {
        return false;
    }
    packages
        .iter()
        .any(|p| changeset_version::is_zero_version(p.version()))
}

pub(super) fn check_early_return(
    changeset_files: &[PathBuf],
    is_graduating: bool,
    input: &ReleaseInput,
    per_package_config: &HashMap<String, PackageReleaseConfig>,
) -> EarlyReturnDecision {
    if changeset_files.is_empty() && !is_graduating {
        if is_any_prerelease_configured(input, per_package_config) && !input.force() {
            return EarlyReturnDecision::ForceRequired;
        }
        return EarlyReturnDecision::NoChangesets;
    }
    EarlyReturnDecision::Continue
}

pub(super) fn collect_unchanged_packages(
    packages: &[PackageInfo],
    planned_releases: &[PackageVersion],
) -> Vec<String> {
    let released: HashSet<&str> = planned_releases.iter().map(|r| r.name().as_str()).collect();

    packages
        .iter()
        .filter(|p| !released.contains(p.name().as_str()))
        .map(|p| p.name().clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mocks::make_package;
    use crate::operations::ReleaseInputBuilder;
    use crate::types::PackageReleaseConfigBuilder;
    use changeset_core::{BumpType, PrereleaseSpec};

    fn default_input() -> ReleaseInput {
        ReleaseInputBuilder::default()
            .no_commit(true)
            .no_tags(true)
            .keep_changesets(true)
            .build()
            .expect("all fields have defaults")
    }

    mod is_prerelease_graduation_tests {
        use super::*;

        #[test]
        fn returns_true_when_package_is_prerelease_and_no_prerelease_config() {
            let packages = vec![make_package("crate-a", "1.0.0-alpha.1")];
            let config = HashMap::new();

            assert!(is_prerelease_graduation(&packages, &config));
        }

        #[test]
        fn returns_false_when_prerelease_config_present() {
            let packages = vec![make_package("crate-a", "1.0.0-alpha.1")];
            let mut config = HashMap::new();
            config.insert(
                "crate-a".to_string(),
                PackageReleaseConfigBuilder::default()
                    .prerelease(Some(PrereleaseSpec::Alpha))
                    .build()
                    .expect("all fields have defaults"),
            );

            assert!(!is_prerelease_graduation(&packages, &config));
        }

        #[test]
        fn returns_false_when_no_prerelease_packages() {
            let packages = vec![make_package("crate-a", "1.0.0")];
            let config = HashMap::new();

            assert!(!is_prerelease_graduation(&packages, &config));
        }

        #[test]
        fn returns_false_for_empty_packages() {
            assert!(!is_prerelease_graduation(&[], &HashMap::new()));
        }
    }

    mod is_zero_graduation_tests {
        use super::*;

        #[test]
        fn returns_true_when_graduate_all_and_zero_package() {
            let packages = vec![make_package("crate-a", "0.5.0")];
            let input = ReleaseInputBuilder::default()
                .no_commit(true)
                .no_tags(true)
                .keep_changesets(true)
                .graduate_all(true)
                .build()
                .expect("all fields have defaults");

            assert!(is_zero_graduation(&packages, &input, &HashMap::new()));
        }

        #[test]
        fn returns_false_when_no_graduation_configured() {
            let packages = vec![make_package("crate-a", "0.5.0")];
            let input = default_input();

            assert!(!is_zero_graduation(&packages, &input, &HashMap::new()));
        }

        #[test]
        fn returns_false_when_no_zero_version_packages() {
            let packages = vec![make_package("crate-a", "1.0.0")];
            let input = ReleaseInputBuilder::default()
                .no_commit(true)
                .no_tags(true)
                .keep_changesets(true)
                .graduate_all(true)
                .build()
                .expect("all fields have defaults");

            assert!(!is_zero_graduation(&packages, &input, &HashMap::new()));
        }

        #[test]
        fn returns_true_when_per_package_graduate_zero() {
            let packages = vec![make_package("crate-a", "0.5.0")];
            let input = default_input();
            let mut config = HashMap::new();
            config.insert(
                "crate-a".to_string(),
                PackageReleaseConfigBuilder::default()
                    .graduate_zero(true)
                    .build()
                    .expect("all fields have defaults"),
            );

            assert!(is_zero_graduation(&packages, &input, &config));
        }
    }

    mod is_any_prerelease_configured_tests {
        use super::*;

        #[test]
        fn returns_true_when_global_prerelease_set() {
            let input = ReleaseInputBuilder::default()
                .no_commit(true)
                .no_tags(true)
                .keep_changesets(true)
                .global_prerelease(Some(PrereleaseSpec::Alpha))
                .build()
                .expect("all fields have defaults");

            assert!(is_any_prerelease_configured(&input, &HashMap::new()));
        }

        #[test]
        fn returns_true_when_per_package_prerelease_set() {
            let input = default_input();
            let mut config = HashMap::new();
            config.insert(
                "crate-a".to_string(),
                PackageReleaseConfigBuilder::default()
                    .prerelease(Some(PrereleaseSpec::Beta))
                    .build()
                    .expect("all fields have defaults"),
            );

            assert!(is_any_prerelease_configured(&input, &config));
        }

        #[test]
        fn returns_false_when_no_prerelease_configured() {
            let input = default_input();

            assert!(!is_any_prerelease_configured(&input, &HashMap::new()));
        }
    }

    mod check_early_return_tests {
        use super::*;

        #[test]
        fn continues_when_changesets_present() {
            let files = vec![PathBuf::from("fix.md")];
            let input = default_input();

            assert!(matches!(
                check_early_return(&files, false, &input, &HashMap::new()),
                EarlyReturnDecision::Continue
            ));
        }

        #[test]
        fn continues_when_graduating() {
            let input = default_input();

            assert!(matches!(
                check_early_return(&[], true, &input, &HashMap::new()),
                EarlyReturnDecision::Continue
            ));
        }

        #[test]
        fn returns_no_changesets_when_empty_and_not_graduating() {
            let input = default_input();

            assert!(matches!(
                check_early_return(&[], false, &input, &HashMap::new()),
                EarlyReturnDecision::NoChangesets
            ));
        }

        #[test]
        fn returns_force_required_when_prerelease_without_force() {
            let input = ReleaseInputBuilder::default()
                .no_commit(true)
                .no_tags(true)
                .keep_changesets(true)
                .global_prerelease(Some(PrereleaseSpec::Alpha))
                .build()
                .expect("all fields have defaults");

            assert!(matches!(
                check_early_return(&[], false, &input, &HashMap::new()),
                EarlyReturnDecision::ForceRequired
            ));
        }

        #[test]
        fn returns_no_changesets_when_prerelease_with_force() {
            let input = ReleaseInputBuilder::default()
                .no_commit(true)
                .no_tags(true)
                .keep_changesets(true)
                .force(true)
                .global_prerelease(Some(PrereleaseSpec::Alpha))
                .build()
                .expect("all fields have defaults");

            assert!(matches!(
                check_early_return(&[], false, &input, &HashMap::new()),
                EarlyReturnDecision::NoChangesets
            ));
        }
    }

    mod collect_unchanged_packages_tests {
        use super::*;

        #[test]
        fn returns_packages_not_in_planned_releases() {
            let packages = vec![
                make_package("crate-a", "1.0.0"),
                make_package("crate-b", "2.0.0"),
                make_package("crate-c", "3.0.0"),
            ];
            let releases = vec![PackageVersion::new(
                "crate-a".to_string(),
                "1.0.0".parse().expect("valid"),
                "1.0.1".parse().expect("valid"),
                BumpType::Patch,
                false,
            )];

            let unchanged = collect_unchanged_packages(&packages, &releases);

            assert_eq!(unchanged, vec!["crate-b", "crate-c"]);
        }

        #[test]
        fn returns_empty_when_all_released() {
            let packages = vec![make_package("crate-a", "1.0.0")];
            let releases = vec![PackageVersion::new(
                "crate-a".to_string(),
                "1.0.0".parse().expect("valid"),
                "1.0.1".parse().expect("valid"),
                BumpType::Patch,
                false,
            )];

            let unchanged = collect_unchanged_packages(&packages, &releases);

            assert!(unchanged.is_empty());
        }

        #[test]
        fn returns_all_when_no_releases() {
            let packages = vec![
                make_package("crate-a", "1.0.0"),
                make_package("crate-b", "2.0.0"),
            ];

            let unchanged = collect_unchanged_packages(&packages, &[]);

            assert_eq!(unchanged, vec!["crate-a", "crate-b"]);
        }
    }
}
