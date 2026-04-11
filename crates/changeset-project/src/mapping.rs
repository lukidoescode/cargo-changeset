use std::collections::HashMap;
use std::hash::BuildHasher;
use std::path::{Path, PathBuf};

use changeset_core::{AdditionalPackageDeclaration, PackageInfo};
use globset::{Glob, GlobSet, GlobSetBuilder};
use gset::Getset;

use crate::config::{PackageChangesetConfig, RootChangesetConfig};
use crate::error::ProjectError;
use crate::project::CargoProject;

#[derive(Debug, Getset)]
pub struct PackageFiles {
    #[getset(get, vis = "pub")]
    package: PackageInfo,
    #[getset(get, vis = "pub")]
    files: Vec<PathBuf>,
}

impl PackageFiles {
    pub(crate) fn new(package: PackageInfo, files: Vec<PathBuf>) -> Self {
        Self { package, files }
    }
}

#[derive(Debug, Default, Getset)]
pub struct FileMapping {
    #[getset(get, vis = "pub")]
    packages: Vec<PackageFiles>,
    #[getset(get, vis = "pub")]
    project: Vec<PathBuf>,
    #[getset(get, vis = "pub")]
    ignored: Vec<PathBuf>,
}

impl FileMapping {
    pub(crate) fn new(
        packages: Vec<PackageFiles>,
        project: Vec<PathBuf>,
        ignored: Vec<PathBuf>,
    ) -> Self {
        Self {
            packages,
            project,
            ignored,
        }
    }

    #[must_use]
    pub fn affected_packages(&self) -> Vec<&PackageInfo> {
        self.packages
            .iter()
            .filter(|pf| !pf.files.is_empty())
            .map(|pf| &pf.package)
            .collect()
    }
}

#[must_use]
pub fn map_files_to_packages<S: BuildHasher>(
    project: &CargoProject,
    changed_files: &[PathBuf],
    root_config: &RootChangesetConfig,
    package_configs: &HashMap<String, PackageChangesetConfig, S>,
) -> FileMapping {
    let mut packages_with_depth: Vec<PackageWithDepth> = project
        .packages()
        .iter()
        .map(|p| {
            let relative_path = p.path().strip_prefix(project.root()).unwrap_or(p.path());
            PackageWithDepth {
                package: p.clone(),
                depth: calculate_path_depth(relative_path),
            }
        })
        .collect();

    packages_with_depth.sort_by(|a, b| b.depth.cmp(&a.depth));

    let mut package_files_map: HashMap<String, Vec<PathBuf>> = HashMap::new();
    let mut project_files = Vec::new();
    let mut ignored_files = Vec::new();

    for file in changed_files {
        if root_config.is_ignored(file) {
            ignored_files.push(file.clone());
            continue;
        }

        let abs_file = if file.is_absolute() {
            file.clone()
        } else {
            project.root().join(file)
        };

        let mut matched = false;
        for pwd in &packages_with_depth {
            if abs_file.starts_with(pwd.package.path()) {
                if let Some(pkg_config) = package_configs.get(pwd.package.name()) {
                    let relative_to_pkg = abs_file
                        .strip_prefix(pwd.package.path())
                        .unwrap_or(&abs_file);
                    if pkg_config.is_ignored(relative_to_pkg) {
                        ignored_files.push(file.clone());
                        matched = true;
                        break;
                    }
                }

                package_files_map
                    .entry(pwd.package.name().clone())
                    .or_default()
                    .push(file.clone());
                matched = true;
                break;
            }
        }

        if !matched {
            project_files.push(file.clone());
        }
    }

    let package_files: Vec<PackageFiles> = project
        .packages()
        .iter()
        .map(|p| {
            PackageFiles::new(
                p.clone(),
                package_files_map.remove(p.name()).unwrap_or_default(),
            )
        })
        .collect();

    FileMapping::new(package_files, project_files, ignored_files)
}

/// # Errors
///
/// Returns `ProjectError::GlobPattern` if any influence pattern in a declaration is invalid.
pub fn compile_influence_patterns(
    declarations: &[AdditionalPackageDeclaration],
) -> Result<Vec<GlobSet>, ProjectError> {
    declarations
        .iter()
        .map(|decl| {
            let mut builder = GlobSetBuilder::new();
            for pattern in decl.influence() {
                let glob = Glob::new(pattern).map_err(|source| ProjectError::GlobPattern {
                    pattern: pattern.clone(),
                    source,
                })?;
                builder.add(glob);
            }
            builder.build().map_err(|source| ProjectError::GlobPattern {
                pattern: decl.influence().join(", "),
                source,
            })
        })
        .collect()
}

#[must_use]
pub fn map_files_to_all_packages<S: BuildHasher>(
    project: &CargoProject,
    changed_files: &[PathBuf],
    root_config: &RootChangesetConfig,
    package_configs: &HashMap<String, PackageChangesetConfig, S>,
    additional_packages: &[(PackageInfo, GlobSet)],
) -> FileMapping {
    let base_mapping = map_files_to_packages(project, changed_files, root_config, package_configs);

    let mut additional_files_map: HashMap<usize, Vec<PathBuf>> = HashMap::new();
    let mut remaining_project_files = Vec::new();

    for file in base_mapping.project() {
        let mut matched = false;
        for (idx, (_, glob_set)) in additional_packages.iter().enumerate() {
            if glob_set.is_match(file) {
                additional_files_map
                    .entry(idx)
                    .or_default()
                    .push(file.clone());
                matched = true;
                break;
            }
        }
        if !matched {
            remaining_project_files.push(file.clone());
        }
    }

    let additional_package_files: Vec<PackageFiles> = additional_packages
        .iter()
        .enumerate()
        .map(|(idx, (pkg, _))| {
            PackageFiles::new(
                pkg.clone(),
                additional_files_map.remove(&idx).unwrap_or_default(),
            )
        })
        .collect();

    let mut all_packages = base_mapping.packages;
    all_packages.extend(additional_package_files);

    FileMapping::new(all_packages, remaining_project_files, base_mapping.ignored)
}

struct PackageWithDepth {
    package: PackageInfo,
    depth: usize,
}

fn calculate_path_depth(path: &Path) -> usize {
    path.components().count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProjectKind;
    use crate::config::parse_root_config;
    use semver::Version;

    fn make_package(name: &str, path: PathBuf) -> PackageInfo {
        PackageInfo::new(name.to_string(), Version::new(0, 1, 0), path)
    }

    fn make_project(root: PathBuf, packages: Vec<PackageInfo>) -> CargoProject {
        CargoProject::new(root, ProjectKind::VirtualWorkspace, packages)
    }

    fn make_glob_set(patterns: &[&str]) -> GlobSet {
        let mut builder = GlobSetBuilder::new();
        for p in patterns {
            builder.add(Glob::new(p).expect("valid glob"));
        }
        builder.build().expect("valid glob set")
    }

    fn make_decl(name: &str, path: &str, influence: &[&str]) -> AdditionalPackageDeclaration {
        let influence_json: Vec<String> = influence.iter().map(|s| format!(r#""{s}""#)).collect();
        let json = format!(
            r#"{{
                "name": "{name}",
                "path": "{path}",
                "influence": [{patterns}],
                "manifest": {{
                    "file-path": "{path}/manifest.yaml",
                    "format": "yaml",
                    "version-path": "version"
                }}
            }}"#,
            patterns = influence_json.join(", ")
        );
        serde_json::from_str(&json).expect("valid declaration JSON")
    }

    #[test]
    fn maps_file_to_correct_package() {
        let root = PathBuf::from("/workspace");
        let pkg_a = make_package("crate-a", root.join("crates/crate-a"));
        let pkg_b = make_package("crate-b", root.join("crates/crate-b"));
        let project = make_project(root.clone(), vec![pkg_a.clone(), pkg_b.clone()]);

        let changed_files = vec![PathBuf::from("crates/crate-a/src/lib.rs")];
        let root_config = RootChangesetConfig::default();
        let package_configs = HashMap::new();

        let mapping =
            map_files_to_packages(&project, &changed_files, &root_config, &package_configs);

        let files_a = mapping
            .packages()
            .iter()
            .find(|pf| pf.package().name() == "crate-a");
        assert!(files_a.is_some());
        assert_eq!(files_a.expect("crate-a should exist").files().len(), 1);

        let files_b = mapping
            .packages()
            .iter()
            .find(|pf| pf.package().name() == "crate-b");
        assert!(files_b.is_some());
        assert!(files_b.expect("crate-b should exist").files().is_empty());
    }

    #[test]
    fn nested_package_takes_precedence() {
        let root = PathBuf::from("/workspace");
        let parent = make_package("parent", root.join("crates/parent"));
        let nested = make_package("nested", root.join("crates/parent/nested"));
        let project = make_project(root.clone(), vec![parent.clone(), nested.clone()]);

        let changed_files = vec![PathBuf::from("crates/parent/nested/src/lib.rs")];
        let root_config = RootChangesetConfig::default();
        let package_configs = HashMap::new();

        let mapping =
            map_files_to_packages(&project, &changed_files, &root_config, &package_configs);

        let nested = mapping
            .packages()
            .iter()
            .find(|pf| pf.package().name() == "nested");
        assert!(nested.is_some());
        assert_eq!(
            nested.expect("nested package should exist").files().len(),
            1
        );

        let parent = mapping
            .packages()
            .iter()
            .find(|pf| pf.package().name() == "parent");
        assert!(parent.is_some());
        assert!(
            parent
                .expect("parent package should exist")
                .files()
                .is_empty()
        );
    }

    #[test]
    fn project_level_files_collected_separately() {
        let root = PathBuf::from("/workspace");
        let pkg = make_package("my-crate", root.join("crates/my-crate"));
        let project = make_project(root.clone(), vec![pkg]);

        let changed_files = vec![
            PathBuf::from("Cargo.toml"),
            PathBuf::from(".github/workflows/ci.yml"),
        ];
        let root_config = RootChangesetConfig::default();
        let package_configs = HashMap::new();

        let mapping =
            map_files_to_packages(&project, &changed_files, &root_config, &package_configs);

        assert_eq!(mapping.project().len(), 2);
        assert!(mapping.project().contains(&PathBuf::from("Cargo.toml")));
    }

    #[test]
    fn affected_packages_returns_only_packages_with_changes() {
        let root = PathBuf::from("/workspace");
        let pkg_a = make_package("crate-a", root.join("crates/crate-a"));
        let pkg_b = make_package("crate-b", root.join("crates/crate-b"));
        let project = make_project(root.clone(), vec![pkg_a, pkg_b]);

        let changed_files = vec![PathBuf::from("crates/crate-a/src/lib.rs")];
        let root_config = RootChangesetConfig::default();
        let package_configs = HashMap::new();

        let mapping =
            map_files_to_packages(&project, &changed_files, &root_config, &package_configs);
        let affected = mapping.affected_packages();

        assert_eq!(affected.len(), 1);
        assert_eq!(affected[0].name(), "crate-a");
    }

    #[test]
    fn empty_project_all_files_are_project_level() {
        let root = PathBuf::from("/workspace");
        let project = make_project(root.clone(), vec![]);

        let changed_files = vec![PathBuf::from("src/lib.rs")];
        let root_config = RootChangesetConfig::default();
        let package_configs = HashMap::new();

        let mapping =
            map_files_to_packages(&project, &changed_files, &root_config, &package_configs);

        assert!(mapping.packages().is_empty());
        assert_eq!(mapping.project().len(), 1);
    }

    #[test]
    fn influence_glob_matches_project_files_to_additional_package() {
        let root = PathBuf::from("/workspace");
        let rust_pkg = make_package("lib", root.join("crates/lib"));
        let project = make_project(root.clone(), vec![rust_pkg]);

        let changed_files = vec![
            PathBuf::from("charts/templates/deployment.yaml"),
            PathBuf::from("crates/lib/src/lib.rs"),
        ];
        let root_config = RootChangesetConfig::default();
        let package_configs = HashMap::new();

        let helm_pkg = make_package("helm-chart", root.join("charts"));
        let glob_set = make_glob_set(&["charts/**"]);
        let additional_packages = vec![(helm_pkg, glob_set)];

        let mapping = map_files_to_all_packages(
            &project,
            &changed_files,
            &root_config,
            &package_configs,
            &additional_packages,
        );

        let helm = mapping
            .packages()
            .iter()
            .find(|pf| pf.package().name() == "helm-chart");
        assert!(helm.is_some());
        assert_eq!(helm.expect("helm-chart should exist").files().len(), 1);
        assert!(
            helm.expect("helm-chart should exist")
                .files()
                .contains(&PathBuf::from("charts/templates/deployment.yaml"))
        );

        let lib_pkg = mapping
            .packages()
            .iter()
            .find(|pf| pf.package().name() == "lib");
        assert!(lib_pkg.is_some());
        assert_eq!(lib_pkg.expect("lib should exist").files().len(), 1);
    }

    #[test]
    fn rust_crate_takes_precedence_over_influence_glob() {
        let root = PathBuf::from("/workspace");
        let rust_pkg = make_package("lib", root.join("crates/lib"));
        let project = make_project(root.clone(), vec![rust_pkg]);

        let changed_files = vec![PathBuf::from("crates/lib/src/lib.rs")];
        let root_config = RootChangesetConfig::default();
        let package_configs = HashMap::new();

        let extra_pkg = make_package("extra", root.join("extra"));
        let glob_set = make_glob_set(&["crates/lib/**"]);
        let additional_packages = vec![(extra_pkg, glob_set)];

        let mapping = map_files_to_all_packages(
            &project,
            &changed_files,
            &root_config,
            &package_configs,
            &additional_packages,
        );

        let lib_pkg = mapping
            .packages()
            .iter()
            .find(|pf| pf.package().name() == "lib");
        assert_eq!(lib_pkg.expect("lib should exist").files().len(), 1);

        let extra = mapping
            .packages()
            .iter()
            .find(|pf| pf.package().name() == "extra");
        assert!(extra.expect("extra should exist").files().is_empty());
    }

    #[test]
    fn files_outside_all_influence_stay_as_project_files() {
        let root = PathBuf::from("/workspace");
        let project = make_project(root.clone(), vec![]);

        let changed_files = vec![PathBuf::from("docs/README.md")];
        let root_config = RootChangesetConfig::default();
        let package_configs = HashMap::new();

        let extra_pkg = make_package("extra", root.join("charts"));
        let glob_set = make_glob_set(&["charts/**"]);
        let additional_packages = vec![(extra_pkg, glob_set)];

        let mapping = map_files_to_all_packages(
            &project,
            &changed_files,
            &root_config,
            &package_configs,
            &additional_packages,
        );

        assert_eq!(mapping.project().len(), 1);
        assert!(mapping.project().contains(&PathBuf::from("docs/README.md")));
    }

    #[test]
    fn multiple_additional_packages_first_match_wins() {
        let root = PathBuf::from("/workspace");
        let project = make_project(root.clone(), vec![]);

        let changed_files = vec![PathBuf::from("shared/foo.yaml")];
        let root_config = RootChangesetConfig::default();
        let package_configs = HashMap::new();

        let pkg_a = make_package("pkg-a", root.join("pkg-a"));
        let glob_a = make_glob_set(&["shared/**"]);
        let pkg_b = make_package("pkg-b", root.join("pkg-b"));
        let glob_b = make_glob_set(&["shared/**"]);
        let additional_packages = vec![(pkg_a, glob_a), (pkg_b, glob_b)];

        let mapping = map_files_to_all_packages(
            &project,
            &changed_files,
            &root_config,
            &package_configs,
            &additional_packages,
        );

        let a = mapping
            .packages()
            .iter()
            .find(|pf| pf.package().name() == "pkg-a");
        assert_eq!(a.expect("pkg-a should exist").files().len(), 1);

        let b = mapping
            .packages()
            .iter()
            .find(|pf| pf.package().name() == "pkg-b");
        assert!(b.expect("pkg-b should exist").files().is_empty());
    }

    #[test]
    fn additional_package_with_no_matching_files_still_in_result() {
        let root = PathBuf::from("/workspace");
        let project = make_project(root.clone(), vec![]);

        let changed_files = vec![PathBuf::from("src/lib.rs")];
        let root_config = RootChangesetConfig::default();
        let package_configs = HashMap::new();

        let helm_pkg = make_package("helm-chart", root.join("charts"));
        let glob_set = make_glob_set(&["charts/**"]);
        let additional_packages = vec![(helm_pkg, glob_set)];

        let mapping = map_files_to_all_packages(
            &project,
            &changed_files,
            &root_config,
            &package_configs,
            &additional_packages,
        );

        let helm = mapping
            .packages()
            .iter()
            .find(|pf| pf.package().name() == "helm-chart");
        assert!(helm.is_some());
        assert!(helm.expect("helm-chart should exist").files().is_empty());
    }

    #[test]
    fn empty_influence_patterns_match_nothing() {
        let root = PathBuf::from("/workspace");
        let project = make_project(root.clone(), vec![]);

        let changed_files = vec![PathBuf::from("charts/foo.yaml")];
        let root_config = RootChangesetConfig::default();
        let package_configs = HashMap::new();

        let pkg = make_package("pkg", root.join("pkg"));
        let glob_set = make_glob_set(&[]);
        let additional_packages = vec![(pkg, glob_set)];

        let mapping = map_files_to_all_packages(
            &project,
            &changed_files,
            &root_config,
            &package_configs,
            &additional_packages,
        );

        let p = mapping
            .packages()
            .iter()
            .find(|pf| pf.package().name() == "pkg");
        assert!(p.expect("pkg should exist").files().is_empty());
        assert_eq!(mapping.project().len(), 1);
    }

    #[test]
    fn ignored_files_not_matched_to_additional_packages() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let root = temp.path();

        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace]\n\n[workspace.metadata.changeset]\nignored-files = [\"*.md\"]\n",
        )
        .expect("write Cargo.toml");

        let cargo_project = make_project(root.to_path_buf(), vec![]);
        let root_config = parse_root_config(&cargo_project).expect("valid config");
        let package_configs = HashMap::new();

        let changed_files = vec![PathBuf::from("charts/README.md")];

        let helm_pkg = make_package("helm-chart", root.join("charts"));
        let glob_set = make_glob_set(&["charts/**"]);
        let additional_packages = vec![(helm_pkg, glob_set)];

        let mapping = map_files_to_all_packages(
            &cargo_project,
            &changed_files,
            &root_config,
            &package_configs,
            &additional_packages,
        );

        assert_eq!(mapping.ignored().len(), 1);
        let helm = mapping
            .packages()
            .iter()
            .find(|pf| pf.package().name() == "helm-chart");
        assert!(helm.expect("helm-chart should exist").files().is_empty());
    }

    #[test]
    fn compile_influence_patterns_valid_patterns() {
        let decl = make_decl("my-chart", "charts/my-chart", &["charts/**", "*.yaml"]);

        let result = compile_influence_patterns(&[decl]).expect("should succeed");

        assert_eq!(result.len(), 1);
        assert!(result[0].is_match("charts/templates/deployment.yaml"));
        assert!(result[0].is_match("values.yaml"));
    }

    #[test]
    fn compile_influence_patterns_invalid_pattern_returns_error() {
        let decl = make_decl("my-chart", "charts/my-chart", &["[invalid"]);

        let result = compile_influence_patterns(&[decl]);

        assert!(matches!(result, Err(ProjectError::GlobPattern { .. })));
    }

    #[test]
    fn compile_influence_patterns_empty_declarations() {
        let result = compile_influence_patterns(&[]).expect("should succeed with empty");
        assert!(result.is_empty());
    }
}
