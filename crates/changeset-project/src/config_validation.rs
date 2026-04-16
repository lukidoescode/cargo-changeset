use std::collections::{HashMap, HashSet};
use std::hash::BuildHasher;

use crate::config::{PackageChangesetConfig, RootChangesetConfig};
use crate::error::ProjectError;

/// # Errors
///
/// Returns `ProjectError` if any version tracking dependency references an unknown package,
/// if circular dependencies are detected, or if duplicate dependency entries exist.
pub fn validate_version_tracking_dependencies<S: BuildHasher, S2: BuildHasher>(
    root_config: &RootChangesetConfig,
    package_configs: &HashMap<String, PackageChangesetConfig, S>,
    all_package_names: &HashSet<String, S2>,
) -> Result<(), ProjectError> {
    let mut seen_pairs: HashSet<(String, String)> = HashSet::new();

    for declaration in root_config.additional_packages() {
        for dep in declaration.dependencies() {
            check_entry(
                declaration.name(),
                dep.dependency_name(),
                all_package_names,
                &mut seen_pairs,
            )?;
        }
    }

    for (package_name, config) in package_configs {
        for dep in config.additional_package_dependencies() {
            check_entry(
                package_name,
                dep.dependency_name(),
                all_package_names,
                &mut seen_pairs,
            )?;
        }
    }

    detect_circular_dependencies(&seen_pairs)
}

fn check_entry<S2: BuildHasher>(
    dependent: &str,
    dependency: &str,
    all_package_names: &HashSet<String, S2>,
    seen_pairs: &mut HashSet<(String, String)>,
) -> Result<(), ProjectError> {
    if !all_package_names.contains(dependency) {
        return Err(ProjectError::UnknownVersionTrackingDependency {
            dependent: dependent.to_string(),
            dependency: dependency.to_string(),
        });
    }

    let pair = (dependent.to_string(), dependency.to_string());
    if !seen_pairs.insert(pair) {
        return Err(ProjectError::DuplicateVersionTrackingDependency {
            dependent: dependent.to_string(),
            dependency: dependency.to_string(),
        });
    }

    Ok(())
}

fn detect_circular_dependencies(edges: &HashSet<(String, String)>) -> Result<(), ProjectError> {
    let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut all_nodes: HashSet<&str> = HashSet::new();

    for (from, to) in edges {
        adjacency
            .entry(from.as_str())
            .or_default()
            .push(to.as_str());
        all_nodes.insert(from.as_str());
        all_nodes.insert(to.as_str());
    }

    let mut visited: HashSet<&str> = HashSet::new();
    let mut in_stack: HashSet<&str> = HashSet::new();

    for &node in &all_nodes {
        if !visited.contains(node)
            && let Some((a, b)) = dfs_find_cycle(node, &adjacency, &mut visited, &mut in_stack)
        {
            let (package_a, package_b) = if a < b { (a, b) } else { (b, a) };
            return Err(ProjectError::CircularVersionTrackingDependency {
                package_a: package_a.to_string(),
                package_b: package_b.to_string(),
            });
        }
    }

    Ok(())
}

fn dfs_find_cycle<'a>(
    node: &'a str,
    adjacency: &HashMap<&'a str, Vec<&'a str>>,
    visited: &mut HashSet<&'a str>,
    in_stack: &mut HashSet<&'a str>,
) -> Option<(&'a str, &'a str)> {
    visited.insert(node);
    in_stack.insert(node);

    if let Some(neighbors) = adjacency.get(node) {
        for &neighbor in neighbors {
            if !visited.contains(neighbor) {
                if let Some(cycle) = dfs_find_cycle(neighbor, adjacency, visited, in_stack) {
                    return Some(cycle);
                }
            } else if in_stack.contains(neighbor) {
                return Some((neighbor, node));
            }
        }
    }

    in_stack.remove(node);
    None
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use changeset_core::{
        AdditionalPackageDeclaration, AdditionalPackageManifest, ManifestFormat,
        VersionTrackingDependency, VersionTrackingManifest,
    };

    use super::*;

    fn make_tracking_manifest() -> VersionTrackingManifest {
        VersionTrackingManifest::new(
            PathBuf::from("tracking/version.json"),
            ManifestFormat::Json,
            "version".to_string(),
        )
    }

    fn make_tracking_dep(dep_name: &str) -> VersionTrackingDependency {
        VersionTrackingDependency::new(dep_name.to_string(), make_tracking_manifest())
    }

    fn make_additional_package(
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
    fn validate_with_known_dependencies_passes() {
        let dep = make_tracking_dep("crate-a");
        let declaration = make_additional_package("my-chart", vec![dep]);
        let root_config =
            RootChangesetConfig::default().with_additional_packages(vec![declaration]);
        let package_configs = HashMap::new();
        let all_names: HashSet<String> = ["my-chart", "crate-a"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();

        let result =
            validate_version_tracking_dependencies(&root_config, &package_configs, &all_names);

        assert!(result.is_ok());
    }

    #[test]
    fn validate_detects_unknown_dependency() {
        let dep = make_tracking_dep("nonexistent-pkg");
        let declaration = make_additional_package("my-chart", vec![dep]);
        let root_config =
            RootChangesetConfig::default().with_additional_packages(vec![declaration]);
        let package_configs = HashMap::new();
        let all_names: HashSet<String> = ["my-chart"].iter().map(|s| (*s).to_string()).collect();

        let result =
            validate_version_tracking_dependencies(&root_config, &package_configs, &all_names);

        assert!(result.is_err());
        let err = result.expect_err("should detect unknown dependency");
        assert!(matches!(
            err,
            ProjectError::UnknownVersionTrackingDependency {
                ref dependency, ..
            } if dependency == "nonexistent-pkg"
        ));
    }

    #[test]
    fn validate_detects_circular_dependency() {
        let dep_a_to_b = make_tracking_dep("pkg-b");
        let declaration_a = make_additional_package("pkg-a", vec![dep_a_to_b]);

        let dep_b_to_a = make_tracking_dep("pkg-a");
        let declaration_b = make_additional_package("pkg-b", vec![dep_b_to_a]);

        let root_config = RootChangesetConfig::default()
            .with_additional_packages(vec![declaration_a, declaration_b]);
        let package_configs = HashMap::new();
        let all_names: HashSet<String> = ["pkg-a", "pkg-b"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();

        let result =
            validate_version_tracking_dependencies(&root_config, &package_configs, &all_names);

        assert!(result.is_err());
        let err = result.expect_err("should detect circular dependency");
        assert!(matches!(
            err,
            ProjectError::CircularVersionTrackingDependency { .. }
        ));
    }

    #[test]
    fn validate_detects_transitive_circular_dependency() {
        let dep_a_to_b = make_tracking_dep("pkg-b");
        let declaration_a = make_additional_package("pkg-a", vec![dep_a_to_b]);

        let dep_b_to_c = make_tracking_dep("pkg-c");
        let declaration_b = make_additional_package("pkg-b", vec![dep_b_to_c]);

        let dep_c_to_a = make_tracking_dep("pkg-a");
        let declaration_c = make_additional_package("pkg-c", vec![dep_c_to_a]);

        let root_config = RootChangesetConfig::default().with_additional_packages(vec![
            declaration_a,
            declaration_b,
            declaration_c,
        ]);
        let package_configs = HashMap::new();
        let all_names: HashSet<String> = ["pkg-a", "pkg-b", "pkg-c"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();

        let result =
            validate_version_tracking_dependencies(&root_config, &package_configs, &all_names);

        assert!(result.is_err());
        let err = result.expect_err("should detect transitive circular dependency");
        assert!(matches!(
            err,
            ProjectError::CircularVersionTrackingDependency { .. }
        ));
    }

    #[test]
    fn validate_detects_duplicate_dependency() {
        let dep1 = make_tracking_dep("crate-a");
        let dep2 = make_tracking_dep("crate-a");
        let declaration = make_additional_package("my-chart", vec![dep1, dep2]);
        let root_config =
            RootChangesetConfig::default().with_additional_packages(vec![declaration]);
        let package_configs = HashMap::new();
        let all_names: HashSet<String> = ["my-chart", "crate-a"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();

        let result =
            validate_version_tracking_dependencies(&root_config, &package_configs, &all_names);

        assert!(result.is_err());
        let err = result.expect_err("should detect duplicate dependency");
        assert!(matches!(
            err,
            ProjectError::DuplicateVersionTrackingDependency {
                ref dependent,
                ref dependency,
            } if dependent == "my-chart" && dependency == "crate-a"
        ));
    }
}
