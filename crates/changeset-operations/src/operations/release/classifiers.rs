use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use changeset_core::PackageInfo;

use super::types::ReleaseInput;
use crate::types::{PackageReleaseConfig, PackageVersion};

pub(super) fn is_any_prerelease_configured(
    input: &ReleaseInput,
    per_package_config: &HashMap<String, PackageReleaseConfig>,
) -> bool {
    input.global_prerelease.is_some() || per_package_config.values().any(|c| c.prerelease.is_some())
}

pub(super) fn is_prerelease_graduation(
    packages: &[PackageInfo],
    per_package_config: &HashMap<String, PackageReleaseConfig>,
) -> bool {
    if per_package_config.values().any(|c| c.prerelease.is_some()) {
        return false;
    }
    packages
        .iter()
        .any(|p| changeset_version::is_prerelease(&p.version))
}

pub(super) fn is_zero_graduation(
    packages: &[PackageInfo],
    input: &ReleaseInput,
    per_package_config: &HashMap<String, PackageReleaseConfig>,
) -> bool {
    let has_graduation = input.graduate_all || per_package_config.values().any(|c| c.graduate_zero);
    if !has_graduation {
        return false;
    }
    packages
        .iter()
        .any(|p| changeset_version::is_zero_version(&p.version))
}

pub(super) enum EarlyReturnDecision {
    Continue,
    NoChangesets,
    ForceRequired,
}

pub(super) fn check_early_return(
    changeset_files: &[PathBuf],
    is_graduating: bool,
    input: &ReleaseInput,
    per_package_config: &HashMap<String, PackageReleaseConfig>,
) -> EarlyReturnDecision {
    if changeset_files.is_empty() && !is_graduating {
        if is_any_prerelease_configured(input, per_package_config) && !input.force {
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
    let released: HashSet<&str> = planned_releases.iter().map(|r| r.name.as_str()).collect();

    packages
        .iter()
        .filter(|p| !released.contains(p.name.as_str()))
        .map(|p| p.name.clone())
        .collect()
}
