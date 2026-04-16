use std::path::{Path, PathBuf};

use changeset_core::{AdditionalPackageDeclaration, CARGO_MANIFEST_FILENAME, PackageInfo};
use globset::GlobBuilder;
use semver::Version;

use crate::CHANGESETS_SUBDIR;
use crate::config::RootChangesetConfig;
use crate::error::ProjectError;
use crate::external_manifest::read_external_version;
use crate::manifest::{CargoManifest, VersionField, read_manifest};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectKind {
    VirtualWorkspace,
    WorkspaceWithRoot,
    SinglePackage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoProject {
    root: PathBuf,
    kind: ProjectKind,
    packages: Vec<PackageInfo>,
}

impl CargoProject {
    #[cfg(any(test, feature = "testing"))]
    #[must_use]
    pub fn new(root: PathBuf, kind: ProjectKind, packages: Vec<PackageInfo>) -> Self {
        Self {
            root,
            kind,
            packages,
        }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn kind(&self) -> &ProjectKind {
        &self.kind
    }

    #[must_use]
    pub fn packages(&self) -> &[PackageInfo] {
        &self.packages
    }
}

/// # Errors
///
/// Returns `ProjectError` if no project root can be found or if manifest parsing fails.
pub fn discover_project(start_dir: &Path) -> Result<CargoProject, ProjectError> {
    let start_dir = start_dir
        .canonicalize()
        .map_err(|source| ProjectError::ManifestRead {
            path: start_dir.to_path_buf(),
            source,
        })?;

    let (root, manifest) = find_project_root(&start_dir)?;
    let kind = determine_project_kind(&manifest);
    let packages = collect_packages(&root, &manifest, &kind)?;

    Ok(CargoProject {
        root,
        kind,
        packages,
    })
}

/// # Errors
///
/// Returns `ProjectError::DirectoryCreate` if directory creation fails.
pub fn ensure_changeset_dir(
    project: &CargoProject,
    config: &RootChangesetConfig,
) -> Result<PathBuf, ProjectError> {
    let changeset_dir = project.root().join(config.changeset_dir());
    let changesets_subdir = changeset_dir.join(CHANGESETS_SUBDIR);
    if !changesets_subdir.exists() {
        std::fs::create_dir_all(&changesets_subdir).map_err(|source| {
            ProjectError::DirectoryCreate {
                path: changesets_subdir,
                source,
            }
        })?;
    }
    Ok(changeset_dir)
}

/// # Errors
///
/// Returns `ProjectError::ManifestRead` if a manifest file cannot be read,
/// or `ProjectError::InvalidVersion` if the version string is not valid semver.
pub fn discover_additional_packages(
    project_root: &Path,
    declarations: &[AdditionalPackageDeclaration],
) -> Result<Vec<PackageInfo>, ProjectError> {
    declarations
        .iter()
        .map(|decl| {
            let manifest_path = project_root.join(decl.manifest().file_path());
            let version = read_external_version(
                &manifest_path,
                decl.manifest().format(),
                decl.manifest().version_field_path(),
            )?;
            Ok(PackageInfo::new(
                decl.name().clone(),
                version,
                project_root.join(decl.path()),
            ))
        })
        .collect()
}

fn find_project_root(start_dir: &Path) -> Result<(PathBuf, CargoManifest), ProjectError> {
    let mut current = start_dir.to_path_buf();
    let mut fallback_single_package: Option<(PathBuf, CargoManifest)> = None;

    loop {
        let manifest_path = current.join(CARGO_MANIFEST_FILENAME);

        if manifest_path.exists() {
            let manifest = read_manifest(&manifest_path)?;

            if manifest.workspace.is_some() {
                return Ok((current, manifest));
            }

            if manifest.package.is_some() && fallback_single_package.is_none() {
                fallback_single_package = Some((current.clone(), manifest));
            }
        }

        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => {
                return fallback_single_package.ok_or_else(|| ProjectError::NotFound {
                    start_dir: start_dir.to_path_buf(),
                });
            }
        }
    }
}

fn determine_project_kind(manifest: &CargoManifest) -> ProjectKind {
    match (&manifest.workspace, &manifest.package) {
        (Some(_), Some(_)) => ProjectKind::WorkspaceWithRoot,
        (None, Some(_)) => ProjectKind::SinglePackage,
        (Some(_) | None, None) => ProjectKind::VirtualWorkspace,
    }
}

fn collect_packages(
    root: &Path,
    manifest: &CargoManifest,
    kind: &ProjectKind,
) -> Result<Vec<PackageInfo>, ProjectError> {
    let workspace_version = manifest
        .workspace
        .as_ref()
        .and_then(|ws| ws.package.as_ref())
        .and_then(|pkg| pkg.version.as_ref());

    let mut packages = Vec::new();

    if *kind == ProjectKind::WorkspaceWithRoot
        && let Some(pkg) = &manifest.package
    {
        let version = resolve_version(
            pkg.version.as_ref(),
            workspace_version,
            &root.join(CARGO_MANIFEST_FILENAME),
        )?;
        packages.push(PackageInfo::new(
            pkg.name.clone(),
            version,
            root.to_path_buf(),
        ));
    }

    if *kind == ProjectKind::SinglePackage
        && let Some(pkg) = &manifest.package
    {
        let version = resolve_version(
            pkg.version.as_ref(),
            workspace_version,
            &root.join(CARGO_MANIFEST_FILENAME),
        )?;
        return Ok(vec![PackageInfo::new(
            pkg.name.clone(),
            version,
            root.to_path_buf(),
        )]);
    }

    if let Some(workspace) = &manifest.workspace {
        let members = workspace.members.as_deref().unwrap_or(&[]);
        let excludes = workspace.exclude.as_deref().unwrap_or(&[]);

        for pattern in members {
            let member_dirs = expand_glob_pattern(root, pattern, excludes)?;

            for member_dir in member_dirs {
                let member_manifest_path = member_dir.join(CARGO_MANIFEST_FILENAME);
                if !member_manifest_path.exists() {
                    continue;
                }

                let member_manifest = read_manifest(&member_manifest_path)?;
                if let Some(pkg) = member_manifest.package {
                    let version = resolve_version(
                        pkg.version.as_ref(),
                        workspace_version,
                        &member_manifest_path,
                    )?;
                    packages.push(PackageInfo::new(pkg.name, version, member_dir));
                }
            }
        }
    }

    Ok(packages)
}

fn resolve_version(
    version_field: Option<&VersionField>,
    workspace_version: Option<&String>,
    manifest_path: &Path,
) -> Result<Version, ProjectError> {
    let version_str = match version_field {
        Some(VersionField::Literal(v)) => v.clone(),
        Some(VersionField::Inherited(inherited)) if inherited.workspace => workspace_version
            .ok_or_else(|| ProjectError::MissingField {
                path: manifest_path.to_path_buf(),
                field: "workspace.package.version",
            })?
            .clone(),
        Some(VersionField::Inherited(_)) | None => {
            return Err(ProjectError::MissingField {
                path: manifest_path.to_path_buf(),
                field: "package.version",
            });
        }
    };

    version_str
        .parse()
        .map_err(|source| ProjectError::InvalidVersion {
            path: manifest_path.to_path_buf(),
            version: version_str,
            source,
        })
}

fn path_to_str(path: &Path) -> Result<&str, ProjectError> {
    path.to_str().ok_or(ProjectError::NonUtf8Path {
        path: path.to_path_buf(),
    })
}

fn expand_glob_pattern(
    root: &Path,
    pattern: &str,
    excludes: &[String],
) -> Result<Vec<PathBuf>, ProjectError> {
    let absolute_pattern = root.join(pattern);
    let pattern_str = path_to_str(&absolute_pattern)?;

    let paths = glob::glob(pattern_str).map_err(|source| ProjectError::GlobPatternParse {
        pattern: pattern.to_string(),
        source,
    })?;

    let exclude_matchers: Vec<globset::GlobMatcher> = excludes
        .iter()
        .map(|ex| {
            GlobBuilder::new(ex)
                .literal_separator(true)
                .build()
                .map(|g| g.compile_matcher())
                .map_err(|source| ProjectError::GlobPattern {
                    pattern: ex.clone(),
                    source,
                })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut dirs = Vec::new();
    for entry in paths {
        let path = entry?;

        if !path.is_dir() {
            continue;
        }

        let relative = path.strip_prefix(root).unwrap_or(&path);

        if exclude_matchers.iter().any(|ex| ex.is_match(relative)) {
            continue;
        }

        dirs.push(path);
    }

    Ok(dirs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_decl(json: &str) -> AdditionalPackageDeclaration {
        serde_json::from_str(json).expect("valid declaration JSON")
    }

    #[test]
    fn discovers_additional_packages_from_declarations() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let root = temp.path();

        std::fs::create_dir_all(root.join("charts/my-chart")).expect("create dir");
        std::fs::write(
            root.join("charts/my-chart/Chart.yaml"),
            "version: \"1.2.3\"\n",
        )
        .expect("write file");

        let decl = make_decl(
            r#"{
            "name": "my-helm-chart",
            "path": "charts/my-chart",
            "influence": ["charts/my-chart/**"],
            "manifest": {
                "file-path": "charts/my-chart/Chart.yaml",
                "format": "yaml",
                "version-field-path": "version"
            }
        }"#,
        );

        let result = discover_additional_packages(root, &[decl]).expect("should succeed");

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name(), "my-helm-chart");
        assert_eq!(result[0].version(), &semver::Version::new(1, 2, 3));
        assert_eq!(*result[0].path(), root.join("charts/my-chart"));
    }

    #[test]
    fn discovers_multiple_additional_packages() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let root = temp.path();

        std::fs::create_dir_all(root.join("charts/chart-a")).expect("create dir");
        std::fs::write(
            root.join("charts/chart-a/Chart.yaml"),
            "version: \"2.0.0\"\n",
        )
        .expect("write yaml");

        std::fs::create_dir_all(root.join("apps/app-b")).expect("create dir");
        std::fs::write(
            root.join("apps/app-b/package.json"),
            r#"{"version": "3.1.0"}"#,
        )
        .expect("write json");

        let decls = vec![
            make_decl(
                r#"{
                "name": "chart-a",
                "path": "charts/chart-a",
                "influence": ["charts/chart-a/**"],
                "manifest": {
                    "file-path": "charts/chart-a/Chart.yaml",
                    "format": "yaml",
                    "version-field-path": "version"
                }
            }"#,
            ),
            make_decl(
                r#"{
                "name": "app-b",
                "path": "apps/app-b",
                "influence": ["apps/app-b/**"],
                "manifest": {
                    "file-path": "apps/app-b/package.json",
                    "format": "json",
                    "version-field-path": "version"
                }
            }"#,
            ),
        ];

        let result = discover_additional_packages(root, &decls).expect("should succeed");

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name(), "chart-a");
        assert_eq!(result[0].version(), &semver::Version::new(2, 0, 0));
        assert_eq!(result[1].name(), "app-b");
        assert_eq!(result[1].version(), &semver::Version::new(3, 1, 0));
    }

    #[test]
    fn returns_error_for_missing_manifest() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let root = temp.path();

        let decl = make_decl(
            r#"{
            "name": "missing-chart",
            "path": "charts/missing",
            "influence": [],
            "manifest": {
                "file-path": "charts/missing/Chart.yaml",
                "format": "yaml",
                "version-field-path": "version"
            }
        }"#,
        );

        let result = discover_additional_packages(root, &[decl]);

        assert!(matches!(result, Err(ProjectError::ManifestRead { .. })));
    }

    #[test]
    fn returns_error_for_invalid_version_in_manifest() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let root = temp.path();

        std::fs::create_dir_all(root.join("charts/bad")).expect("create dir");
        std::fs::write(
            root.join("charts/bad/Chart.yaml"),
            "version: \"not-a-version\"\n",
        )
        .expect("write file");

        let decl = make_decl(
            r#"{
            "name": "bad-chart",
            "path": "charts/bad",
            "influence": [],
            "manifest": {
                "file-path": "charts/bad/Chart.yaml",
                "format": "yaml",
                "version-field-path": "version"
            }
        }"#,
        );

        let result = discover_additional_packages(root, &[decl]);

        assert!(matches!(result, Err(ProjectError::InvalidVersion { .. })));
    }

    #[test]
    fn returns_empty_vec_for_no_declarations() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let result =
            discover_additional_packages(temp.path(), &[]).expect("should succeed with empty");
        assert!(result.is_empty());
    }

    #[test]
    fn determine_project_kind_virtual() {
        let manifest = CargoManifest {
            package: None,
            workspace: Some(crate::manifest::WorkspaceSection {
                members: Some(vec!["crates/*".to_string()]),
                exclude: None,
                package: None,
                metadata: None,
            }),
            dependencies: None,
            build_dependencies: None,
        };
        assert_eq!(
            determine_project_kind(&manifest),
            ProjectKind::VirtualWorkspace
        );
    }

    #[test]
    fn determine_project_kind_workspace_with_root() {
        let manifest = CargoManifest {
            package: Some(crate::manifest::Package {
                name: "test".to_string(),
                version: Some(VersionField::Literal("1.0.0".to_string())),
                metadata: None,
            }),
            workspace: Some(crate::manifest::WorkspaceSection {
                members: Some(vec!["crates/*".to_string()]),
                exclude: None,
                package: None,
                metadata: None,
            }),
            dependencies: None,
            build_dependencies: None,
        };
        assert_eq!(
            determine_project_kind(&manifest),
            ProjectKind::WorkspaceWithRoot
        );
    }

    #[test]
    fn determine_project_kind_single_package() {
        let manifest = CargoManifest {
            package: Some(crate::manifest::Package {
                name: "test".to_string(),
                version: Some(VersionField::Literal("1.0.0".to_string())),
                metadata: None,
            }),
            workspace: None,
            dependencies: None,
            build_dependencies: None,
        };
        assert_eq!(
            determine_project_kind(&manifest),
            ProjectKind::SinglePackage
        );
    }

    #[test]
    fn expand_glob_returns_only_directories() -> Result<(), ProjectError> {
        let temp = tempfile::tempdir().expect("create temp dir");
        let root = temp.path();

        std::fs::create_dir_all(root.join("crates/foo")).expect("create dir");
        std::fs::write(root.join("crates/bar.txt"), "file").expect("create file");

        let result = expand_glob_pattern(root, "crates/*", &[])?;

        assert_eq!(result.len(), 1);
        assert!(result[0].ends_with("crates/foo"));
        Ok(())
    }

    #[test]
    fn expand_glob_no_matches_returns_empty() -> Result<(), ProjectError> {
        let temp = tempfile::tempdir().expect("create temp dir");
        let root = temp.path();

        std::fs::create_dir_all(root.join("other")).expect("create dir");

        let result = expand_glob_pattern(root, "crates/*", &[])?;

        assert!(result.is_empty());
        Ok(())
    }

    #[test]
    fn expand_glob_excludes_matching_dirs() -> Result<(), ProjectError> {
        let temp = tempfile::tempdir().expect("create temp dir");
        let root = temp.path();

        std::fs::create_dir_all(root.join("crates/included")).expect("create dir");
        std::fs::create_dir_all(root.join("crates/excluded")).expect("create dir");

        let excludes = vec!["crates/excluded".to_string()];
        let result = expand_glob_pattern(root, "crates/*", &excludes)?;

        assert_eq!(result.len(), 1);
        assert!(result[0].ends_with("crates/included"));
        Ok(())
    }

    #[test]
    fn expand_glob_literal_pattern() -> Result<(), ProjectError> {
        let temp = tempfile::tempdir().expect("create temp dir");
        let root = temp.path();

        std::fs::create_dir_all(root.join("specific-crate")).expect("create dir");

        let result = expand_glob_pattern(root, "specific-crate", &[])?;

        assert_eq!(result.len(), 1);
        assert!(result[0].ends_with("specific-crate"));
        Ok(())
    }

    #[test]
    fn expand_glob_star_does_not_match_nested() -> Result<(), ProjectError> {
        let temp = tempfile::tempdir().expect("create temp dir");
        let root = temp.path();

        std::fs::create_dir_all(root.join("crates/foo")).expect("create dir");
        std::fs::create_dir_all(root.join("crates/foo/nested")).expect("create dir");

        let result = expand_glob_pattern(root, "crates/*", &[])?;

        assert_eq!(result.len(), 1);
        assert!(result[0].ends_with("crates/foo"));
        Ok(())
    }

    #[test]
    fn expand_glob_invalid_pattern_returns_error() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let result = expand_glob_pattern(temp.path(), "[invalid", &[]);

        assert!(matches!(
            result,
            Err(ProjectError::GlobPatternParse { pattern, .. }) if pattern == "[invalid"
        ));
    }

    #[test]
    fn expand_glob_double_star_matches_nested() -> Result<(), ProjectError> {
        let temp = tempfile::tempdir().expect("create temp dir");
        let root = temp.path();

        std::fs::create_dir_all(root.join("packages/foo")).expect("create dir");
        std::fs::create_dir_all(root.join("packages/bar/nested")).expect("create dir");

        let result = expand_glob_pattern(root, "packages/**", &[])?;

        let relative_paths: Vec<_> = result
            .iter()
            .map(|p| p.strip_prefix(root).expect("strip prefix"))
            .collect();

        assert!(relative_paths.iter().any(|p| p.ends_with("foo")));
        assert!(relative_paths.iter().any(|p| p.ends_with("nested")));
        Ok(())
    }

    #[test]
    fn expand_glob_invalid_exclude_pattern_returns_error() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let root = temp.path();

        std::fs::create_dir_all(root.join("crates/foo")).expect("create dir");

        let excludes = vec!["[invalid".to_string()];
        let result = expand_glob_pattern(root, "crates/*", &excludes);

        assert!(matches!(
            result,
            Err(ProjectError::GlobPattern { pattern, .. }) if pattern == "[invalid"
        ));
    }
}
