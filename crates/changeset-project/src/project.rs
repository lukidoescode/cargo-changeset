use std::path::{Path, PathBuf};

use changeset_core::PackageInfo;
use globset::GlobBuilder;
use semver::Version;

use crate::config::RootChangesetConfig;
use crate::error::ProjectError;
use crate::manifest::{CargoManifest, VersionField, read_manifest};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectKind {
    VirtualWorkspace,
    WorkspaceWithRoot,
    SinglePackage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoProject {
    pub root: PathBuf,
    pub kind: ProjectKind,
    pub packages: Vec<PackageInfo>,
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
/// Returns `ProjectError` if no project root can be found or if manifest parsing fails.
pub fn discover_project_from_cwd() -> Result<CargoProject, ProjectError> {
    let cwd = std::env::current_dir()?;
    discover_project(&cwd)
}

/// # Errors
///
/// Returns `ProjectError::Io` if directory creation fails.
pub fn ensure_changeset_dir(
    project: &CargoProject,
    config: &RootChangesetConfig,
) -> Result<PathBuf, ProjectError> {
    let changeset_dir = project.root.join(config.changeset_dir());
    if !changeset_dir.exists() {
        std::fs::create_dir_all(&changeset_dir)?;
    }
    Ok(changeset_dir)
}

fn find_project_root(start_dir: &Path) -> Result<(PathBuf, CargoManifest), ProjectError> {
    let mut current = start_dir.to_path_buf();
    let mut fallback_single_package: Option<(PathBuf, CargoManifest)> = None;

    loop {
        let manifest_path = current.join("Cargo.toml");

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

    if *kind == ProjectKind::WorkspaceWithRoot {
        if let Some(pkg) = &manifest.package {
            let version = resolve_version(
                pkg.version.as_ref(),
                workspace_version,
                &root.join("Cargo.toml"),
            )?;
            packages.push(PackageInfo {
                name: pkg.name.clone(),
                version,
                path: root.to_path_buf(),
            });
        }
    }

    if *kind == ProjectKind::SinglePackage {
        if let Some(pkg) = &manifest.package {
            let version = resolve_version(
                pkg.version.as_ref(),
                workspace_version,
                &root.join("Cargo.toml"),
            )?;
            return Ok(vec![PackageInfo {
                name: pkg.name.clone(),
                version,
                path: root.to_path_buf(),
            }]);
        }
    }

    if let Some(workspace) = &manifest.workspace {
        let members = workspace.members.as_deref().unwrap_or(&[]);
        let excludes = workspace.exclude.as_deref().unwrap_or(&[]);

        for pattern in members {
            let member_dirs = expand_glob_pattern(root, pattern, excludes)?;

            for member_dir in member_dirs {
                let member_manifest_path = member_dir.join("Cargo.toml");
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
                    packages.push(PackageInfo {
                        name: pkg.name,
                        version,
                        path: member_dir,
                    });
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

fn expand_glob_pattern(
    root: &Path,
    pattern: &str,
    excludes: &[String],
) -> Result<Vec<PathBuf>, ProjectError> {
    let glob = GlobBuilder::new(pattern)
        .literal_separator(true)
        .build()
        .map_err(|source| ProjectError::GlobPattern {
            pattern: pattern.to_string(),
            source,
        })?
        .compile_matcher();

    let exclude_matchers: Vec<_> = excludes
        .iter()
        .filter_map(|ex| {
            GlobBuilder::new(ex)
                .literal_separator(true)
                .build()
                .ok()
                .map(|g| g.compile_matcher())
        })
        .collect();

    let mut dirs = Vec::new();
    collect_matching_dirs(root, root, &glob, &exclude_matchers, &mut dirs)?;

    Ok(dirs)
}

fn collect_matching_dirs(
    base: &Path,
    current: &Path,
    glob: &globset::GlobMatcher,
    excludes: &[globset::GlobMatcher],
    results: &mut Vec<PathBuf>,
) -> Result<(), ProjectError> {
    let entries = std::fs::read_dir(current)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        // Fallback to full path if strip_prefix fails (shouldn't happen in practice)
        let relative = path.strip_prefix(base).unwrap_or(&path);

        if excludes.iter().any(|ex| ex.is_match(relative)) {
            continue;
        }

        if glob.is_match(relative) {
            results.push(path.clone());
        }

        collect_matching_dirs(base, &path, glob, excludes, results)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_project_kind_virtual() {
        let manifest = CargoManifest {
            package: None,
            workspace: Some(crate::manifest::WorkspaceSection {
                members: Some(vec!["crates/*".to_string()]),
                exclude: None,
                package: None,
                metadata: None,
            }),
        };
        assert_eq!(
            determine_project_kind(&manifest),
            ProjectKind::VirtualWorkspace
        );
    }

    #[test]
    fn test_determine_project_kind_workspace_with_root() {
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
        };
        assert_eq!(
            determine_project_kind(&manifest),
            ProjectKind::WorkspaceWithRoot
        );
    }

    #[test]
    fn test_determine_project_kind_single_package() {
        let manifest = CargoManifest {
            package: Some(crate::manifest::Package {
                name: "test".to_string(),
                version: Some(VersionField::Literal("1.0.0".to_string())),
                metadata: None,
            }),
            workspace: None,
        };
        assert_eq!(
            determine_project_kind(&manifest),
            ProjectKind::SinglePackage
        );
    }
}
