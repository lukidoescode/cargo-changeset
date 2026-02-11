use std::collections::HashSet;
use std::path::PathBuf;

use changeset_git::{FileStatus, Repository};
use changeset_parse::parse_changeset;
use changeset_project::{discover_project_from_cwd, load_changeset_configs, map_files_to_packages};

use super::VerifyArgs;
use crate::error::{CliError, Result};

pub(crate) fn run(args: VerifyArgs) -> Result<()> {
    let project = discover_project_from_cwd()?;
    let repo = Repository::open(&project.root)?;

    let (root_config, package_configs) = load_changeset_configs(&project)?;
    let changeset_dir = root_config.changeset_dir();

    let head_ref = args.head.as_deref().unwrap_or("HEAD");
    let changed_files = repo.changed_files(Some(&args.base), head_ref)?;

    let (changeset_changes, code_changes): (Vec<_>, Vec<_>) = changed_files
        .into_iter()
        .partition(|change| change.path.starts_with(changeset_dir));

    let deleted_changesets: Vec<PathBuf> = changeset_changes
        .iter()
        .filter_map(|change| {
            let is_md = |p: &std::path::Path| p.extension().is_some_and(|ext| ext == "md");

            match change.status {
                FileStatus::Deleted if is_md(&change.path) => Some(change.path.clone()),
                FileStatus::Renamed => change
                    .old_path
                    .as_ref()
                    .filter(|old| old.starts_with(changeset_dir) && is_md(old))
                    .cloned(),
                _ => None,
            }
        })
        .collect();

    if !deleted_changesets.is_empty() && !args.allow_deleted_changesets {
        return Err(CliError::ChangesetDeleted {
            paths: deleted_changesets,
        });
    }

    let changeset_files_in_diff: Vec<PathBuf> = changeset_changes
        .into_iter()
        .filter(|change| {
            change.path.extension().is_some_and(|ext| ext == "md")
                && matches!(
                    change.status,
                    FileStatus::Added | FileStatus::Modified | FileStatus::Renamed
                )
        })
        .map(|change| change.path)
        .collect();

    let changed_paths: Vec<PathBuf> = code_changes.into_iter().map(|change| change.path).collect();

    if changed_paths.is_empty() {
        if !args.quiet {
            println!("No files changed (excluding {})", changeset_dir.display());
        }
        return Ok(());
    }

    let mapping = map_files_to_packages(&project, &changed_paths, &root_config, &package_configs);

    let affected_packages = mapping.affected_packages();

    if affected_packages.is_empty() {
        if !args.quiet {
            println!("No packages affected by changes");
            if !mapping.project_files.is_empty() {
                println!(
                    "  {} project-level file(s) changed",
                    mapping.project_files.len()
                );
            }
            if !mapping.ignored_files.is_empty() {
                println!(
                    "  {} file(s) ignored by patterns",
                    mapping.ignored_files.len()
                );
            }
        }
        return Ok(());
    }

    let covered_packages = get_covered_packages(&project.root, &changeset_files_in_diff)?;

    let uncovered: Vec<_> = affected_packages
        .iter()
        .filter(|pkg| !covered_packages.contains(&pkg.name))
        .collect();

    if !args.quiet {
        println!("Changed packages:");
        for pkg in &affected_packages {
            let status = if covered_packages.contains(&pkg.name) {
                "✓"
            } else {
                "✗"
            };
            println!("  {status} {}", pkg.name);
        }

        if !mapping.project_files.is_empty() {
            println!("\nProject-level files:");
            for file in &mapping.project_files {
                println!("  {}", file.display());
            }
        }

        if !mapping.ignored_files.is_empty() {
            println!("\nIgnored files:");
            for file in &mapping.ignored_files {
                println!("  {}", file.display());
            }
        }

        if !covered_packages.is_empty() {
            println!("\nChangesets cover:");
            for name in &covered_packages {
                println!("  {name}");
            }
        }
    }

    if uncovered.is_empty() {
        if !args.quiet {
            println!("\nAll changed packages have changeset coverage");
        }
        Ok(())
    } else {
        if !args.quiet {
            eprintln!("Packages without changeset coverage:");
            for pkg in &uncovered {
                eprintln!("  {}", pkg.name);
            }
        }
        Err(CliError::VerificationFailed {
            uncovered_count: uncovered.len(),
        })
    }
}

fn get_covered_packages(
    project_root: &std::path::Path,
    changeset_files: &[PathBuf],
) -> Result<HashSet<String>> {
    let mut covered = HashSet::new();

    for relative_path in changeset_files {
        let path = project_root.join(relative_path);

        let content =
            std::fs::read_to_string(&path).map_err(|source| CliError::ChangesetFileRead {
                path: path.clone(),
                source,
            })?;

        let changeset = parse_changeset(&content).map_err(|source| CliError::ChangesetParse {
            path: path.clone(),
            source,
        })?;

        for release in changeset.releases {
            covered.insert(release.name);
        }
    }

    Ok(covered)
}
