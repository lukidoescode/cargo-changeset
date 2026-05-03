use std::collections::{HashMap, HashSet, VecDeque};

use changeset_core::CARGO_MANIFEST_FILENAME;

use crate::error::ProjectError;
use crate::manifest::{DependencyEntry, read_manifest};
use crate::project::CargoProject;

pub struct WorkspaceDependencyGraph {
    depended_on_by: HashMap<String, HashSet<String>>,
    depends_on: HashMap<String, HashSet<String>>,
}

impl WorkspaceDependencyGraph {
    /// Builds the dependency graph from the workspace, considering `[dependencies]`,
    /// `[build-dependencies]`, and their target-specific equivalents under
    /// `[target.'...'.dependencies]` and `[target.'...'.build-dependencies]`.
    ///
    /// # Errors
    ///
    /// Returns `ProjectError` if any member's manifest cannot be read or parsed.
    pub fn build(project: &CargoProject) -> Result<Self, ProjectError> {
        let member_names: HashSet<String> = project
            .packages()
            .iter()
            .map(|p| p.name().clone())
            .collect();

        let mut depended_on_by: HashMap<String, HashSet<String>> = member_names
            .iter()
            .map(|name| (name.clone(), HashSet::<String>::new()))
            .collect();

        let mut depends_on: HashMap<String, HashSet<String>> = member_names
            .iter()
            .map(|name| (name.clone(), HashSet::<String>::new()))
            .collect();

        for package in project.packages() {
            let manifest_path = package.path().join(CARGO_MANIFEST_FILENAME);
            let manifest = read_manifest(&manifest_path)?;

            let dep_sections = [manifest.dependencies, manifest.build_dependencies];

            for section in dep_sections.into_iter().flatten() {
                for (key, entry) in &section {
                    let resolved_name = resolve_package_name(key, entry);

                    if member_names.contains(resolved_name) {
                        if let Some(set) = depends_on.get_mut(package.name()) {
                            set.insert(resolved_name.to_string());
                        }
                        if let Some(set) = depended_on_by.get_mut(resolved_name) {
                            set.insert(package.name().clone());
                        }
                    }
                }
            }

            if let Some(ref target_map) = manifest.target {
                for target_deps in target_map.values() {
                    let target_sections = [
                        target_deps.dependencies.as_ref(),
                        target_deps.build_dependencies.as_ref(),
                    ];
                    for section in target_sections.into_iter().flatten() {
                        for (key, entry) in section {
                            let resolved_name = resolve_package_name(key, entry);
                            if member_names.contains(resolved_name) {
                                if let Some(set) = depends_on.get_mut(package.name()) {
                                    set.insert(resolved_name.to_string());
                                }
                                if let Some(set) = depended_on_by.get_mut(resolved_name) {
                                    set.insert(package.name().clone());
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(Self {
            depended_on_by,
            depends_on,
        })
    }

    #[cfg(any(test, feature = "testing"))]
    #[must_use]
    pub fn from_edges(member_names: &HashSet<String>, edges: &[(String, String)]) -> Self {
        let mut depended_on_by: HashMap<String, HashSet<String>> = member_names
            .iter()
            .map(|name| (name.clone(), HashSet::new()))
            .collect();

        let mut depends_on: HashMap<String, HashSet<String>> = member_names
            .iter()
            .map(|name| (name.clone(), HashSet::new()))
            .collect();

        for (dependent, dependency) in edges {
            if member_names.contains(dependent) && member_names.contains(dependency) {
                depends_on
                    .entry(dependent.clone())
                    .or_default()
                    .insert(dependency.clone());

                depended_on_by
                    .entry(dependency.clone())
                    .or_default()
                    .insert(dependent.clone());
            }
        }

        Self {
            depended_on_by,
            depends_on,
        }
    }

    #[must_use]
    pub fn transitive_dependents(&self, package: &str) -> HashSet<&str> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        if let Some(direct) = self.depended_on_by.get(package) {
            for dep in direct {
                if dep != package && visited.insert(dep.as_str()) {
                    queue.push_back(dep.as_str());
                }
            }
        }

        while let Some(current) = queue.pop_front() {
            if let Some(dependents) = self.depended_on_by.get(current) {
                for dep in dependents {
                    if dep != package && visited.insert(dep.as_str()) {
                        queue.push_back(dep.as_str());
                    }
                }
            }
        }

        visited
    }

    #[must_use]
    pub fn transitive_dependents_of_set<'a>(&'a self, packages: &[&str]) -> HashSet<&'a str> {
        let input_set: HashSet<&str> = packages.iter().copied().collect();

        let mut result = HashSet::new();
        for &pkg in packages {
            for dep in self.transitive_dependents(pkg) {
                if !input_set.contains(dep) {
                    result.insert(dep);
                }
            }
        }

        result
    }

    pub fn add_members(&mut self, names: impl IntoIterator<Item = String>) {
        for name in names {
            self.depended_on_by.entry(name.clone()).or_default();
            self.depends_on.entry(name).or_default();
        }
    }

    pub fn extend_with_edges(&mut self, edges: &[(String, String)]) {
        for (dependent, dependency) in edges {
            if self.depended_on_by.contains_key(dependent.as_str())
                && self.depended_on_by.contains_key(dependency.as_str())
            {
                self.depends_on
                    .entry(dependent.clone())
                    .or_default()
                    .insert(dependency.clone());
                self.depended_on_by
                    .entry(dependency.clone())
                    .or_default()
                    .insert(dependent.clone());
            }
        }
    }

    #[must_use]
    pub fn direct_dependencies(&self, package: &str) -> HashSet<&str> {
        self.depends_on
            .get(package)
            .map(|deps| deps.iter().map(String::as_str).collect())
            .unwrap_or_default()
    }
}

fn resolve_package_name<'a>(key: &'a str, entry: &'a DependencyEntry) -> &'a str {
    match entry {
        DependencyEntry::Table(table) => table.package.as_deref().unwrap_or(key),
        DependencyEntry::Simple(_) => key,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member_set(names: &[&str]) -> HashSet<String> {
        names.iter().map(|&n| n.to_string()).collect()
    }

    fn edges(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|&(a, b)| (a.to_string(), b.to_string()))
            .collect()
    }

    #[test]
    fn empty_graph_has_no_dependents() {
        let graph = WorkspaceDependencyGraph::from_edges(&member_set(&["a", "b"]), &[]);

        assert!(graph.transitive_dependents("a").is_empty());
        assert!(graph.transitive_dependents("b").is_empty());
    }

    #[test]
    fn single_package_no_edges() {
        let graph = WorkspaceDependencyGraph::from_edges(&member_set(&["solo"]), &[]);

        assert!(graph.transitive_dependents("solo").is_empty());
        assert!(graph.direct_dependencies("solo").is_empty());
    }

    #[test]
    fn direct_dependency_detection() {
        let graph = WorkspaceDependencyGraph::from_edges(
            &member_set(&["app", "lib"]),
            &edges(&[("app", "lib")]),
        );

        let deps = graph.direct_dependencies("app");
        assert_eq!(deps, HashSet::from(["lib"]));
        assert!(graph.direct_dependencies("lib").is_empty());
    }

    #[test]
    fn transitive_dependents_chain() {
        let graph = WorkspaceDependencyGraph::from_edges(
            &member_set(&["a", "b", "c"]),
            &edges(&[("b", "a"), ("c", "b")]),
        );

        let dependents = graph.transitive_dependents("a");
        assert!(dependents.contains("b"));
        assert!(dependents.contains("c"));
        assert_eq!(dependents.len(), 2);
    }

    #[test]
    fn transitive_dependents_diamond() {
        let graph = WorkspaceDependencyGraph::from_edges(
            &member_set(&["core", "left", "right", "top"]),
            &edges(&[
                ("left", "core"),
                ("right", "core"),
                ("top", "left"),
                ("top", "right"),
            ]),
        );

        let dependents = graph.transitive_dependents("core");
        assert!(dependents.contains("left"));
        assert!(dependents.contains("right"));
        assert!(dependents.contains("top"));
        assert_eq!(dependents.len(), 3);
    }

    #[test]
    fn transitive_dependents_of_set_excludes_input() {
        let graph = WorkspaceDependencyGraph::from_edges(
            &member_set(&["a", "b", "c", "d"]),
            &edges(&[("b", "a"), ("c", "b"), ("d", "c")]),
        );

        let result = graph.transitive_dependents_of_set(&["a", "b"]);
        assert!(result.contains("c"));
        assert!(result.contains("d"));
        assert!(!result.contains("a"));
        assert!(!result.contains("b"));
    }

    #[test]
    fn transitive_dependents_of_set_deduplicates_shared_dependent() {
        let graph = WorkspaceDependencyGraph::from_edges(
            &member_set(&["a", "b", "shared"]),
            &edges(&[("shared", "a"), ("shared", "b")]),
        );

        let result = graph.transitive_dependents_of_set(&["a", "b"]);
        assert_eq!(result.len(), 1);
        assert!(result.contains("shared"));
    }

    #[test]
    fn cycle_handling_terminates() {
        let graph = WorkspaceDependencyGraph::from_edges(
            &member_set(&["a", "b"]),
            &edges(&[("a", "b"), ("b", "a")]),
        );

        let dependents_a = graph.transitive_dependents("a");
        assert!(dependents_a.contains("b"));
        assert!(!dependents_a.contains("a"));

        let dependents_b = graph.transitive_dependents("b");
        assert!(dependents_b.contains("a"));
        assert!(!dependents_b.contains("b"));
    }

    #[test]
    fn cycle_three_nodes() {
        let graph = WorkspaceDependencyGraph::from_edges(
            &member_set(&["a", "b", "c"]),
            &edges(&[("a", "b"), ("b", "c"), ("c", "a")]),
        );

        let dependents = graph.transitive_dependents("a");
        assert!(dependents.contains("b"));
        assert!(dependents.contains("c"));
    }

    #[test]
    fn unknown_package_returns_empty() {
        let graph = WorkspaceDependencyGraph::from_edges(&member_set(&["a"]), &[]);

        assert!(graph.transitive_dependents("nonexistent").is_empty());
        assert!(graph.direct_dependencies("nonexistent").is_empty());
    }

    #[test]
    fn from_edges_ignores_non_member_edges() {
        let graph = WorkspaceDependencyGraph::from_edges(
            &member_set(&["a", "b"]),
            &edges(&[("a", "external"), ("external", "b")]),
        );

        assert!(graph.direct_dependencies("a").is_empty());
        assert!(graph.transitive_dependents("b").is_empty());
    }

    #[test]
    fn resolve_package_name_simple_entry() {
        let entry = DependencyEntry::Simple(serde::de::IgnoredAny);
        assert_eq!(resolve_package_name("my-dep", &entry), "my-dep");
    }

    #[test]
    fn resolve_package_name_table_without_rename() {
        let entry = DependencyEntry::Table(crate::manifest::DependencyTable { package: None });
        assert_eq!(resolve_package_name("my-dep", &entry), "my-dep");
    }

    #[test]
    fn resolve_package_name_table_with_rename() {
        let entry = DependencyEntry::Table(crate::manifest::DependencyTable {
            package: Some("actual-name".to_string()),
        });
        assert_eq!(resolve_package_name("alias", &entry), "actual-name");
    }

    #[test]
    fn extend_with_edges_adds_forward_and_reverse_edges() {
        let mut graph = WorkspaceDependencyGraph::from_edges(
            &member_set(&["a", "b", "c"]),
            &edges(&[("b", "a")]),
        );

        graph.extend_with_edges(&edges(&[("c", "b")]));

        let deps_c = graph.direct_dependencies("c");
        assert_eq!(deps_c, HashSet::from(["b"]));

        let dependents_b = graph.transitive_dependents("b");
        assert!(dependents_b.contains("c"));
    }

    #[test]
    fn extend_with_edges_ignores_unknown_members() {
        let mut graph =
            WorkspaceDependencyGraph::from_edges(&member_set(&["a", "b"]), &edges(&[("b", "a")]));

        graph.extend_with_edges(&edges(&[("a", "unknown"), ("unknown", "b")]));

        assert!(graph.direct_dependencies("a").is_empty());
        assert_eq!(graph.direct_dependencies("b"), HashSet::from(["a"]));
        assert!(graph.transitive_dependents("b").is_empty());
    }

    #[test]
    fn add_members_then_extend_with_edges_allows_cross_type_dependencies() {
        let mut graph = WorkspaceDependencyGraph::from_edges(&member_set(&["rust-crate"]), &[]);

        graph.add_members(vec!["helm-chart".to_string()]);
        graph.extend_with_edges(&edges(&[("helm-chart", "rust-crate")]));

        let deps = graph.direct_dependencies("helm-chart");
        assert_eq!(deps, HashSet::from(["rust-crate"]));
    }

    #[test]
    fn transitive_dependents_works_across_extended_edges() {
        let mut graph = WorkspaceDependencyGraph::from_edges(
            &member_set(&["core", "lib"]),
            &edges(&[("lib", "core")]),
        );

        graph.add_members(vec!["chart".to_string()]);
        graph.extend_with_edges(&edges(&[("chart", "lib")]));

        let result = graph.transitive_dependents_of_set(&["core"]);
        assert!(result.contains("lib"));
        assert!(result.contains("chart"));
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn add_members_is_idempotent_for_existing_names() {
        let mut graph =
            WorkspaceDependencyGraph::from_edges(&member_set(&["a", "b"]), &edges(&[("a", "b")]));

        graph.add_members(vec!["a".to_string()]);

        assert_eq!(graph.direct_dependencies("a"), HashSet::from(["b"]));
        assert_eq!(graph.transitive_dependents("b"), HashSet::from(["a"]));
    }

    #[test]
    fn add_members_with_empty_iterator_is_noop() {
        let mut graph =
            WorkspaceDependencyGraph::from_edges(&member_set(&["a", "b"]), &edges(&[("a", "b")]));

        graph.add_members(Vec::<String>::new());

        assert_eq!(graph.direct_dependencies("a"), HashSet::from(["b"]));
    }
}
