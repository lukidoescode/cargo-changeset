use std::path::{Path, PathBuf};

use changeset_git::{FileChange, FileStatus, Repository};
use changeset_project::{
    FileMapping, discover_project, load_changeset_configs, map_files_to_packages,
};

use super::VerifyArgs;
use crate::error::{CliError, Result};
use crate::output::{OutputFormatter, PlainTextFormatter};
use crate::verification::VerificationContext;
use crate::verification::engine::VerificationEngine;
use crate::verification::reader::FileSystemChangesetReader;
use crate::verification::rules::{CoverageRule, DeletedChangesetsRule};

fn is_markdown_file(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "md")
}

fn extract_deleted_changesets(changes: &[FileChange], changeset_dir: &Path) -> Vec<PathBuf> {
    changes
        .iter()
        .filter_map(|change| match change.status {
            FileStatus::Deleted if is_markdown_file(&change.path) => Some(change.path.clone()),
            FileStatus::Renamed => change
                .old_path
                .as_ref()
                .filter(|old| old.starts_with(changeset_dir) && is_markdown_file(old))
                .cloned(),
            _ => None,
        })
        .collect()
}

fn extract_active_changesets(changes: &[FileChange]) -> Vec<PathBuf> {
    changes
        .iter()
        .filter(|change| {
            is_markdown_file(&change.path)
                && matches!(
                    change.status,
                    FileStatus::Added | FileStatus::Modified | FileStatus::Renamed
                )
        })
        .map(|change| change.path.clone())
        .collect()
}

fn build_context(
    mapping: Option<&FileMapping>,
    changeset_files: Vec<PathBuf>,
    deleted_changesets: Vec<PathBuf>,
) -> VerificationContext {
    match mapping {
        Some(m) => VerificationContext {
            affected_packages: m.affected_packages().into_iter().cloned().collect(),
            changeset_files,
            deleted_changesets,
            project_files: m.project_files.clone(),
            ignored_files: m.ignored_files.clone(),
        },
        None => VerificationContext {
            affected_packages: Vec::new(),
            changeset_files,
            deleted_changesets,
            project_files: Vec::new(),
            ignored_files: Vec::new(),
        },
    }
}

pub(crate) fn run(args: VerifyArgs, start_path: &Path) -> Result<()> {
    let project = discover_project(start_path)?;
    let repo = Repository::open(&project.root)?;
    let (root_config, package_configs) = load_changeset_configs(&project)?;
    let changeset_dir = root_config.changeset_dir();

    let head_ref = args.head.as_deref().unwrap_or("HEAD");
    let changed_files = repo.changed_files(Some(&args.base), head_ref)?;

    let (changeset_changes, code_changes): (Vec<_>, Vec<_>) = changed_files
        .into_iter()
        .partition(|change| change.path.starts_with(changeset_dir));

    let deleted_changesets = extract_deleted_changesets(&changeset_changes, changeset_dir);
    let changeset_files = extract_active_changesets(&changeset_changes);

    let changed_paths: Vec<PathBuf> = code_changes.into_iter().map(|change| change.path).collect();

    let has_deleted_changesets = !deleted_changesets.is_empty();
    let has_code_changes = !changed_paths.is_empty();

    if !has_code_changes && !has_deleted_changesets {
        if !args.quiet {
            println!("No files changed (excluding {})", changeset_dir.display());
        }
        return Ok(());
    }

    let mapping = if has_code_changes {
        Some(map_files_to_packages(
            &project,
            &changed_paths,
            &root_config,
            &package_configs,
        ))
    } else {
        None
    };

    let affected_packages = mapping
        .as_ref()
        .map_or(Vec::new(), |m| m.affected_packages());

    if affected_packages.is_empty() && !has_deleted_changesets {
        if !args.quiet {
            println!("No packages affected by changes");
            if let Some(m) = &mapping {
                if !m.project_files.is_empty() {
                    println!("  {} project-level file(s) changed", m.project_files.len());
                }
                if !m.ignored_files.is_empty() {
                    println!("  {} file(s) ignored by patterns", m.ignored_files.len());
                }
            }
        }
        return Ok(());
    }

    let context = build_context(mapping.as_ref(), changeset_files, deleted_changesets);

    let reader = FileSystemChangesetReader::new(&project.root);
    let deleted_rule = DeletedChangesetsRule::new(args.allow_deleted_changesets);
    let coverage_rule = CoverageRule::new(&reader);

    let mut engine = VerificationEngine::new();
    engine.add_rule(&deleted_rule);
    engine.add_rule(&coverage_rule);

    let result = engine.verify(&context)?;

    let formatter = PlainTextFormatter;
    if !args.quiet {
        if result.is_success() {
            print!("{}", formatter.format_success(&result));
        } else {
            eprint!("{}", formatter.format_failure(&result));
        }
    }

    if result.is_success() {
        Ok(())
    } else if !result.deleted_changesets.is_empty() {
        Err(CliError::ChangesetDeleted {
            paths: result.deleted_changesets,
        })
    } else {
        Err(CliError::VerificationFailed {
            uncovered_count: result.uncovered_packages.len(),
        })
    }
}
