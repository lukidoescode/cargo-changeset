use std::collections::HashMap;

use changeset_core::{PackageInfo, PrereleaseSpec};
use changeset_project::GraduationState;
use changeset_version::is_zero_version;

use super::validator::ReleaseCliInput;
use crate::types::PackageReleaseConfig;

#[derive(Debug, Clone)]
pub struct ValidatedReleaseConfig {
    per_package: HashMap<String, PackageReleaseConfig>,
}

impl ValidatedReleaseConfig {
    #[must_use]
    pub fn per_package(&self) -> &HashMap<String, PackageReleaseConfig> {
        &self.per_package
    }

    #[must_use]
    pub fn into_per_package(self) -> HashMap<String, PackageReleaseConfig> {
        self.per_package
    }
}

#[derive(Default)]
pub(super) struct ParsedPrereleaseCache {
    pub(super) specs: HashMap<String, PrereleaseSpec>,
}

pub(super) struct ReleaseConfigBuilder;

impl ReleaseConfigBuilder {
    pub(super) fn build(
        cli_input: &ReleaseCliInput,
        parsed_cache: &ParsedPrereleaseCache,
        graduation_state: Option<&GraduationState>,
        packages: &[PackageInfo],
    ) -> ValidatedReleaseConfig {
        let mut per_package = HashMap::new();

        for (pkg, spec) in &parsed_cache.specs {
            per_package
                .entry(pkg.clone())
                .or_insert_with(PackageReleaseConfig::default)
                .prerelease = Some(spec.clone());
        }

        for (pkg, spec) in &cli_input.cli_prerelease {
            per_package
                .entry(pkg.clone())
                .or_insert_with(PackageReleaseConfig::default)
                .prerelease = Some(spec.clone());
        }

        if let Some(global) = &cli_input.global_prerelease {
            for pkg in packages {
                per_package
                    .entry(pkg.name.clone())
                    .or_insert_with(PackageReleaseConfig::default)
                    .prerelease = Some(global.clone());
            }
        }

        if let Some(state) = graduation_state {
            for pkg in state.iter() {
                per_package
                    .entry(pkg.to_string())
                    .or_insert_with(PackageReleaseConfig::default)
                    .graduate_zero = true;
            }
        }

        for pkg in &cli_input.cli_graduate {
            per_package
                .entry(pkg.clone())
                .or_insert_with(PackageReleaseConfig::default)
                .graduate_zero = true;
        }

        if cli_input.graduate_all {
            for pkg in packages {
                if is_zero_version(&pkg.version) {
                    per_package
                        .entry(pkg.name.clone())
                        .or_insert_with(PackageReleaseConfig::default)
                        .graduate_zero = true;
                }
            }
        }

        ValidatedReleaseConfig { per_package }
    }
}
