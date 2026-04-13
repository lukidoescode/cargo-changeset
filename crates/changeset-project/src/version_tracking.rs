use std::collections::HashMap;
use std::hash::BuildHasher;

use changeset_core::VersionTrackingManifest;
use gset::Getset;

use crate::config::{PackageChangesetConfig, RootChangesetConfig};

#[derive(Debug, Getset)]
pub struct ResolvedVersionTracking {
    #[getset(get, vis = "pub")]
    dependent_name: String,
    #[getset(get, vis = "pub")]
    dependency_name: String,
    #[getset(get, vis = "pub")]
    manifest: VersionTrackingManifest,
}

impl ResolvedVersionTracking {
    #[cfg(any(test, feature = "testing"))]
    #[must_use]
    pub fn new(
        dependent_name: String,
        dependency_name: String,
        manifest: VersionTrackingManifest,
    ) -> Self {
        Self {
            dependent_name,
            dependency_name,
            manifest,
        }
    }

    #[cfg(not(any(test, feature = "testing")))]
    #[must_use]
    pub(crate) fn new(
        dependent_name: String,
        dependency_name: String,
        manifest: VersionTrackingManifest,
    ) -> Self {
        Self {
            dependent_name,
            dependency_name,
            manifest,
        }
    }
}

#[must_use]
pub fn collect_version_tracking_info<S: BuildHasher>(
    root_config: &RootChangesetConfig,
    package_configs: &HashMap<String, PackageChangesetConfig, S>,
) -> Vec<ResolvedVersionTracking> {
    let mut entries = Vec::new();

    for declaration in root_config.additional_packages() {
        for dep in declaration.dependencies() {
            entries.push(ResolvedVersionTracking {
                dependent_name: declaration.name().clone(),
                dependency_name: dep.dependency_name().clone(),
                manifest: dep.version_tracking_manifest().clone(),
            });
        }
    }

    for (package_name, config) in package_configs {
        for dep in config.additional_package_dependencies() {
            entries.push(ResolvedVersionTracking {
                dependent_name: package_name.clone(),
                dependency_name: dep.dependency_name().clone(),
                manifest: dep.version_tracking_manifest().clone(),
            });
        }
    }

    entries
}

#[must_use]
pub fn tracking_edges(entries: &[ResolvedVersionTracking]) -> Vec<(String, String)> {
    entries
        .iter()
        .map(|e| (e.dependent_name().clone(), e.dependency_name().clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use changeset_core::{
        AdditionalPackageDeclaration, AdditionalPackageManifest, ManifestFormat,
        VersionTrackingDependency, VersionTrackingManifest,
    };

    use super::*;

    fn make_version_tracking_manifest() -> VersionTrackingManifest {
        VersionTrackingManifest::new(
            PathBuf::from("tracking/version.json"),
            ManifestFormat::Json,
            "version".to_string(),
        )
    }

    fn make_version_tracking_dep(dep_name: &str) -> VersionTrackingDependency {
        VersionTrackingDependency::new(dep_name.to_string(), make_version_tracking_manifest())
    }

    fn make_additional_package_declaration(
        name: &str,
        deps: Vec<VersionTrackingDependency>,
    ) -> AdditionalPackageDeclaration {
        AdditionalPackageDeclaration::new(
            name.to_string(),
            PathBuf::from(format!("packages/{name}")),
            vec![format!("packages/{name}/**")],
            AdditionalPackageManifest::new(
                PathBuf::from(format!("packages/{name}/manifest.yaml")),
                ManifestFormat::Yaml,
                "version".to_string(),
            ),
            deps,
        )
    }

    #[test]
    fn collect_from_additional_packages_only() {
        let dep = make_version_tracking_dep("upstream-service");
        let declaration = make_additional_package_declaration("my-helm-chart", vec![dep]);
        let root_config =
            RootChangesetConfig::default().with_additional_packages(vec![declaration]);
        let package_configs = HashMap::new();

        let result = collect_version_tracking_info(&root_config, &package_configs);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].dependent_name(), "my-helm-chart");
        assert_eq!(result[0].dependency_name(), "upstream-service");
    }

    #[test]
    fn collect_from_cargo_crate_deps_only() {
        let root_config = RootChangesetConfig::default();
        let dep = make_version_tracking_dep("my-lib");
        let package_config =
            PackageChangesetConfig::default().with_additional_package_dependencies(vec![dep]);
        let mut package_configs = HashMap::new();
        package_configs.insert("my-crate".to_string(), package_config);

        let result = collect_version_tracking_info(&root_config, &package_configs);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].dependent_name(), "my-crate");
        assert_eq!(result[0].dependency_name(), "my-lib");
    }

    #[test]
    fn collect_from_both_sources() {
        let additional_dep = make_version_tracking_dep("upstream-service");
        let declaration =
            make_additional_package_declaration("my-helm-chart", vec![additional_dep]);
        let root_config =
            RootChangesetConfig::default().with_additional_packages(vec![declaration]);

        let cargo_dep = make_version_tracking_dep("my-lib");
        let mut package_configs = HashMap::new();
        package_configs.insert(
            "my-crate".to_string(),
            PackageChangesetConfig::default().with_additional_package_dependencies(vec![cargo_dep]),
        );

        let result = collect_version_tracking_info(&root_config, &package_configs);

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn tracking_edges_extracts_pairs() {
        let entries = vec![
            ResolvedVersionTracking::new(
                "dependent-a".to_string(),
                "dependency-x".to_string(),
                make_version_tracking_manifest(),
            ),
            ResolvedVersionTracking::new(
                "dependent-b".to_string(),
                "dependency-y".to_string(),
                make_version_tracking_manifest(),
            ),
        ];

        let edges = tracking_edges(&entries);

        assert_eq!(edges.len(), 2);
        assert!(edges.contains(&("dependent-a".to_string(), "dependency-x".to_string())));
        assert!(edges.contains(&("dependent-b".to_string(), "dependency-y".to_string())));
    }

    #[test]
    fn empty_inputs_return_empty_vec() {
        let root_config = RootChangesetConfig::default();
        let package_configs = HashMap::new();

        let result = collect_version_tracking_info(&root_config, &package_configs);

        assert!(result.is_empty());
        assert!(tracking_edges(&result).is_empty());
    }
}
