use std::collections::HashMap;
use std::hash::BuildHasher;
use std::path::{Path, PathBuf};

use changeset_core::PackageInfo;
use gset::Getset;

use crate::config::{PackageChangesetConfig, RootChangesetConfig};
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

struct PackageWithDepth {
    package: PackageInfo,
    depth: usize,
}

fn calculate_path_depth(path: &Path) -> usize {
    path.components().count()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProjectKind;
    use semver::Version;

    fn make_package(name: &str, path: PathBuf) -> PackageInfo {
        PackageInfo::new(name.to_string(), Version::new(0, 1, 0), path)
    }

    fn make_project(root: PathBuf, packages: Vec<PackageInfo>) -> CargoProject {
        CargoProject::new(root, ProjectKind::VirtualWorkspace, packages)
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
}
