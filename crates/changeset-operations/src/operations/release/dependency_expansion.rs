use std::collections::HashSet;

use changeset_core::{BumpType, PackageInfo, ZeroVersionBehavior};
use changeset_project::WorkspaceDependencyGraph;
use changeset_version::{VersionError, calculate_new_version_with_zero_behavior};

use crate::types::PackageVersion;

pub(crate) fn expand_with_reverse_dependencies(
    initial_releases: Vec<PackageVersion>,
    graph: &WorkspaceDependencyGraph,
    packages: &[PackageInfo],
    zero_behavior: ZeroVersionBehavior,
) -> Result<Vec<PackageVersion>, VersionError> {
    let initial_names: HashSet<String> = initial_releases.iter().map(|r| r.name.clone()).collect();

    let initial_refs: Vec<&str> = initial_names.iter().map(String::as_str).collect();
    let dependents = graph.transitive_dependents_of_set(&initial_refs);

    let mut result = initial_releases;

    for dep_name in dependents {
        if initial_names.contains(dep_name) {
            continue;
        }

        if let Some(pkg) = packages.iter().find(|p| p.name() == dep_name) {
            let new_version = calculate_new_version_with_zero_behavior(
                pkg.version(),
                Some(BumpType::Patch),
                None,
                zero_behavior,
                false,
            )?;
            result.push(PackageVersion {
                name: pkg.name().clone(),
                current_version: pkg.version().clone(),
                new_version,
                bump_type: BumpType::Patch,
                auto_bumped: true,
            });
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use changeset_core::{BumpType, PackageInfo, ZeroVersionBehavior};
    use changeset_project::WorkspaceDependencyGraph;
    use semver::Version;

    use super::*;

    fn make_package(name: &str, version: &str) -> PackageInfo {
        PackageInfo::new(
            name.to_string(),
            version.parse().expect("valid version"),
            std::path::PathBuf::from(format!("crates/{name}")),
        )
    }

    fn make_release(name: &str, current: &str, new: &str, bump: BumpType) -> PackageVersion {
        PackageVersion {
            name: name.to_string(),
            current_version: current.parse().expect("valid version"),
            new_version: new.parse().expect("valid version"),
            bump_type: bump,
            auto_bumped: false,
        }
    }

    fn make_graph(members: &[&str], edges: &[(&str, &str)]) -> WorkspaceDependencyGraph {
        let member_set: HashSet<String> = members.iter().map(|&n| n.to_string()).collect();
        let edge_vec: Vec<(String, String)> = edges
            .iter()
            .map(|&(a, b)| (a.to_string(), b.to_string()))
            .collect();
        WorkspaceDependencyGraph::from_edges(member_set, &edge_vec)
    }

    #[test]
    fn no_reverse_dependencies_returns_input() {
        let packages = vec![make_package("a", "1.0.0"), make_package("b", "1.0.0")];
        let graph = make_graph(&["a", "b"], &[]);
        let releases = vec![make_release("a", "1.0.0", "1.0.1", BumpType::Patch)];

        let result = expand_with_reverse_dependencies(
            releases.clone(),
            &graph,
            &packages,
            ZeroVersionBehavior::EffectiveMinor,
        )
        .expect("should succeed");

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "a");
    }

    #[test]
    fn single_direct_dependent_gets_patch_bump() {
        let packages = vec![make_package("core", "1.0.0"), make_package("app", "2.0.0")];
        let graph = make_graph(&["core", "app"], &[("app", "core")]);
        let releases = vec![make_release("core", "1.0.0", "1.1.0", BumpType::Minor)];

        let result = expand_with_reverse_dependencies(
            releases,
            &graph,
            &packages,
            ZeroVersionBehavior::EffectiveMinor,
        )
        .expect("should succeed");

        assert_eq!(result.len(), 2);
        assert_eq!(result[1].name, "app");
        assert_eq!(result[1].new_version, Version::new(2, 0, 1));
        assert_eq!(result[1].bump_type, BumpType::Patch);
        assert!(result[1].auto_bumped);
    }

    #[test]
    fn transitive_chain_bumps_all() {
        let packages = vec![
            make_package("a", "1.0.0"),
            make_package("b", "1.0.0"),
            make_package("c", "1.0.0"),
        ];
        let graph = make_graph(&["a", "b", "c"], &[("b", "a"), ("c", "b")]);
        let releases = vec![make_release("a", "1.0.0", "1.1.0", BumpType::Minor)];

        let result = expand_with_reverse_dependencies(
            releases,
            &graph,
            &packages,
            ZeroVersionBehavior::EffectiveMinor,
        )
        .expect("should succeed");

        assert_eq!(result.len(), 3);
        assert!(!result[0].auto_bumped);

        let auto_names: HashSet<&str> = result
            .iter()
            .filter(|r| r.auto_bumped)
            .map(|r| r.name.as_str())
            .collect();
        assert!(auto_names.contains("b"));
        assert!(auto_names.contains("c"));
    }

    #[test]
    fn diamond_dependency_no_duplicates() {
        let packages = vec![
            make_package("core", "1.0.0"),
            make_package("left", "1.0.0"),
            make_package("right", "1.0.0"),
            make_package("top", "1.0.0"),
        ];
        let graph = make_graph(
            &["core", "left", "right", "top"],
            &[
                ("left", "core"),
                ("right", "core"),
                ("top", "left"),
                ("top", "right"),
            ],
        );
        let releases = vec![make_release("core", "1.0.0", "1.1.0", BumpType::Minor)];

        let result = expand_with_reverse_dependencies(
            releases,
            &graph,
            &packages,
            ZeroVersionBehavior::EffectiveMinor,
        )
        .expect("should succeed");

        let names: Vec<&str> = result.iter().map(|r| r.name.as_str()).collect();
        let unique_names: HashSet<&str> = names.iter().copied().collect();
        assert_eq!(names.len(), unique_names.len());
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn already_covered_dependent_not_duplicated() {
        let packages = vec![make_package("core", "1.0.0"), make_package("app", "2.0.0")];
        let graph = make_graph(&["core", "app"], &[("app", "core")]);
        let releases = vec![
            make_release("core", "1.0.0", "1.1.0", BumpType::Minor),
            make_release("app", "2.0.0", "2.1.0", BumpType::Minor),
        ];

        let result = expand_with_reverse_dependencies(
            releases,
            &graph,
            &packages,
            ZeroVersionBehavior::EffectiveMinor,
        )
        .expect("should succeed");

        assert_eq!(result.len(), 2);
        let app_release = result.iter().find(|r| r.name == "app").expect("app exists");
        assert_eq!(app_release.new_version, Version::new(2, 1, 0));
        assert!(!app_release.auto_bumped);
    }

    #[test]
    fn empty_graph_returns_input_unchanged() {
        let packages = vec![make_package("a", "1.0.0")];
        let graph = make_graph(&["a"], &[]);
        let releases = vec![make_release("a", "1.0.0", "1.0.1", BumpType::Patch)];

        let result = expand_with_reverse_dependencies(
            releases.clone(),
            &graph,
            &packages,
            ZeroVersionBehavior::EffectiveMinor,
        )
        .expect("should succeed");

        assert_eq!(result, releases);
    }

    #[test]
    fn auto_bumped_flag_correctness() {
        let packages = vec![make_package("lib", "1.0.0"), make_package("dep", "1.0.0")];
        let graph = make_graph(&["lib", "dep"], &[("dep", "lib")]);
        let releases = vec![make_release("lib", "1.0.0", "1.1.0", BumpType::Minor)];

        let result = expand_with_reverse_dependencies(
            releases,
            &graph,
            &packages,
            ZeroVersionBehavior::EffectiveMinor,
        )
        .expect("should succeed");

        let lib_release = result.iter().find(|r| r.name == "lib").expect("lib exists");
        let dep_release = result.iter().find(|r| r.name == "dep").expect("dep exists");

        assert!(!lib_release.auto_bumped);
        assert!(dep_release.auto_bumped);
    }
}
