use std::collections::HashMap;

use changeset_core::{PackageInfo, PrereleaseSpec};
use changeset_project::GraduationState;
use changeset_version::is_zero_version;

use super::validator::ReleaseCliInput;
use crate::types::PackageReleaseConfig;

#[derive(Debug, Clone)]
#[must_use]
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

pub(super) fn build_release_config(
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
            .set_prerelease(spec.clone());
    }

    for (pkg, spec) in &cli_input.cli_prerelease {
        per_package
            .entry(pkg.clone())
            .or_insert_with(PackageReleaseConfig::default)
            .set_prerelease(spec.clone());
    }

    if let Some(global) = cli_input.global_prerelease.as_ref() {
        for pkg in packages {
            per_package
                .entry(pkg.name().clone())
                .or_insert_with(PackageReleaseConfig::default)
                .set_prerelease(global.clone());
        }
    }

    if let Some(state) = graduation_state {
        for pkg in state.iter() {
            per_package
                .entry(pkg.to_string())
                .or_insert_with(PackageReleaseConfig::default)
                .set_graduate_zero();
        }
    }

    for pkg in &cli_input.cli_graduate {
        per_package
            .entry(pkg.clone())
            .or_insert_with(PackageReleaseConfig::default)
            .set_graduate_zero();
    }

    if cli_input.graduate_all {
        for pkg in packages {
            if is_zero_version(pkg.version()) {
                per_package
                    .entry(pkg.name().clone())
                    .or_insert_with(PackageReleaseConfig::default)
                    .set_graduate_zero();
            }
        }
    }

    ValidatedReleaseConfig { per_package }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mocks::make_package;
    use changeset_project::GraduationState;

    fn empty_cli_input() -> ReleaseCliInput {
        ReleaseCliInput::default()
    }

    #[test]
    fn empty_inputs_produce_empty_config() {
        let cli_input = empty_cli_input();
        let cache = ParsedPrereleaseCache::default();
        let packages = vec![make_package("crate-a", "1.0.0")];

        let config = build_release_config(&cli_input, &cache, None, &packages);

        assert!(config.per_package().is_empty());
    }

    #[test]
    fn toml_prerelease_specs_are_applied() {
        let cli_input = empty_cli_input();
        let mut cache = ParsedPrereleaseCache::default();
        cache
            .specs
            .insert("crate-a".to_string(), PrereleaseSpec::Alpha);
        let packages = vec![make_package("crate-a", "1.0.0")];

        let config = build_release_config(&cli_input, &cache, None, &packages);

        assert_eq!(
            config.per_package()["crate-a"].prerelease(),
            Some(&PrereleaseSpec::Alpha)
        );
    }

    #[test]
    fn cli_prerelease_overrides_toml() {
        let mut cli_input = empty_cli_input();
        cli_input
            .cli_prerelease
            .insert("crate-a".to_string(), PrereleaseSpec::Beta);
        let mut cache = ParsedPrereleaseCache::default();
        cache
            .specs
            .insert("crate-a".to_string(), PrereleaseSpec::Alpha);
        let packages = vec![make_package("crate-a", "1.0.0")];

        let config = build_release_config(&cli_input, &cache, None, &packages);

        assert_eq!(
            config.per_package()["crate-a"].prerelease(),
            Some(&PrereleaseSpec::Beta)
        );
    }

    #[test]
    fn global_prerelease_applies_to_all_packages() {
        let mut cli_input = empty_cli_input();
        cli_input.global_prerelease = Some(PrereleaseSpec::Rc);
        let cache = ParsedPrereleaseCache::default();
        let packages = vec![
            make_package("crate-a", "1.0.0"),
            make_package("crate-b", "2.0.0"),
        ];

        let config = build_release_config(&cli_input, &cache, None, &packages);

        assert_eq!(
            config.per_package()["crate-a"].prerelease(),
            Some(&PrereleaseSpec::Rc)
        );
        assert_eq!(
            config.per_package()["crate-b"].prerelease(),
            Some(&PrereleaseSpec::Rc)
        );
    }

    #[test]
    fn graduation_state_marks_packages() {
        let cli_input = empty_cli_input();
        let cache = ParsedPrereleaseCache::default();
        let mut state = GraduationState::default();
        state.add("crate-a".to_string());
        let packages = vec![make_package("crate-a", "0.5.0")];

        let config = build_release_config(&cli_input, &cache, Some(&state), &packages);

        assert!(config.per_package()["crate-a"].graduate_zero());
    }

    #[test]
    fn graduate_all_marks_zero_version_packages() {
        let mut cli_input = empty_cli_input();
        cli_input.graduate_all = true;
        let cache = ParsedPrereleaseCache::default();
        let packages = vec![
            make_package("crate-a", "0.5.0"),
            make_package("crate-b", "1.0.0"),
        ];

        let config = build_release_config(&cli_input, &cache, None, &packages);

        assert!(config.per_package()["crate-a"].graduate_zero());
        assert!(!config.per_package().contains_key("crate-b"));
    }

    #[test]
    fn cli_graduate_marks_specific_packages() {
        let mut cli_input = empty_cli_input();
        cli_input.cli_graduate.insert("crate-a".to_string());
        let cache = ParsedPrereleaseCache::default();
        let packages = vec![make_package("crate-a", "0.5.0")];

        let config = build_release_config(&cli_input, &cache, None, &packages);

        assert!(config.per_package()["crate-a"].graduate_zero());
    }
}
