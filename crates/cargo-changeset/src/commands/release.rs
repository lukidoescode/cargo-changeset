use std::collections::{HashMap, HashSet};
use std::path::Path;

use changeset_core::PrereleaseSpec;
use changeset_operations::OperationError;
use changeset_operations::operations::{
    GitOperationResult, PackageReleaseConfig, ReleaseInput, ReleaseOperation, ReleaseOutcome,
    ReleaseOutput,
};
use changeset_operations::providers::{
    FileSystemChangelogWriter, FileSystemChangesetIO, FileSystemManifestWriter,
    FileSystemProjectProvider, FileSystemReleaseStateIO, Git2Provider,
};
use changeset_operations::traits::ProjectProvider;
use changeset_version::is_prerelease;

use super::ReleaseArgs;
use crate::error::Result;

/// Parsed prerelease specification from CLI
#[derive(Debug, Clone)]
pub(crate) struct ParsedPrereleaseArgs {
    /// Per-package prerelease tags (crate_name -> tag)
    pub per_package: HashMap<String, PrereleaseSpec>,
    /// Global prerelease tag (applies to all if per_package is empty)
    pub global: Option<PrereleaseSpec>,
}

/// Parsed graduation specification from CLI
#[derive(Debug, Clone)]
struct ParsedGraduateArgs {
    /// Specific packages to graduate
    packages: HashSet<String>,
    /// Whether `--graduate` was passed without specific crates
    all: bool,
}

pub(crate) fn run(args: ReleaseArgs, start_path: &Path) -> Result<()> {
    let project_provider = FileSystemProjectProvider::new();
    let project = project_provider.discover_project(start_path)?;
    let changeset_io = FileSystemChangesetIO::new(&project.root);
    let manifest_writer = FileSystemManifestWriter::new();
    let changelog_writer = FileSystemChangelogWriter::new();
    let git_provider = Git2Provider::new();
    let release_state_io = FileSystemReleaseStateIO::new();

    let parsed_prerelease = parse_prerelease_args(&args.prerelease, &project)?;
    let parsed_graduate = parse_graduate_args(&args.graduate);

    let mut per_package_config = HashMap::new();
    if let Some(ref parsed) = parsed_prerelease {
        for (pkg, spec) in &parsed.per_package {
            per_package_config
                .entry(pkg.clone())
                .or_insert_with(PackageReleaseConfig::default)
                .prerelease = Some(spec.clone());
        }
    }
    for pkg in &parsed_graduate.packages {
        per_package_config
            .entry(pkg.clone())
            .or_insert_with(PackageReleaseConfig::default)
            .graduate_zero = true;
    }

    let operation = ReleaseOperation::new(
        project_provider,
        changeset_io,
        manifest_writer,
        changelog_writer,
        git_provider,
        release_state_io,
    );
    let input = ReleaseInput {
        dry_run: args.dry_run,
        convert_inherited: args.convert,
        no_commit: args.no_commit,
        no_tags: args.no_tags,
        keep_changesets: args.keep_changesets,
        force: args.force,
        per_package_config,
        global_prerelease: parsed_prerelease.and_then(|p| p.global),
        graduate_all: parsed_graduate.all,
    };
    let outcome = operation.execute(start_path, &input)?;

    print_outcome(&outcome);

    Ok(())
}

fn parse_prerelease_args(
    args: &[String],
    project: &changeset_project::CargoProject,
) -> Result<Option<ParsedPrereleaseArgs>> {
    if args.is_empty() {
        return Ok(None);
    }

    let mut per_package = HashMap::new();
    let mut global = None;

    for arg in args {
        if arg.is_empty() {
            // Empty string means infer from existing prerelease
            let has_prerelease = project.packages.iter().any(|p| is_prerelease(&p.version));
            if has_prerelease {
                let first_prerelease = project
                    .packages
                    .iter()
                    .find(|p| is_prerelease(&p.version))
                    .and_then(|p| changeset_version::extract_prerelease_tag(&p.version));

                if let Some(existing_tag) = first_prerelease {
                    global = Some(parse_prerelease_spec(&existing_tag)?);
                    continue;
                }
            }
            return Err(OperationError::PrereleaseTagRequired.into());
        }

        if let Some((crate_name, tag)) = arg.split_once(':') {
            // Per-package format: "crate:tag"
            let spec = parse_prerelease_spec(tag)?;
            per_package.insert(crate_name.to_string(), spec);
        } else {
            // Global format: just "tag"
            global = Some(parse_prerelease_spec(arg)?);
        }
    }

    Ok(Some(ParsedPrereleaseArgs {
        per_package,
        global,
    }))
}

fn parse_prerelease_spec(s: &str) -> Result<PrereleaseSpec> {
    s.parse()
        .map_err(|_| crate::error::CliError::InvalidPrereleaseTag { tag: s.to_string() })
}

fn parse_graduate_args(args: &[String]) -> ParsedGraduateArgs {
    if args.is_empty() {
        return ParsedGraduateArgs {
            packages: HashSet::new(),
            all: false,
        };
    }

    let packages: HashSet<String> = args.iter().filter(|s| !s.is_empty()).cloned().collect();

    let all = args.iter().any(std::string::String::is_empty);

    ParsedGraduateArgs { packages, all }
}

fn print_outcome(outcome: &ReleaseOutcome) {
    match outcome {
        ReleaseOutcome::NoChangesets => {
            println!("No pending changesets to release.");
        }
        ReleaseOutcome::DryRun(output) => {
            println!("Dry run - no changes will be made.\n");
            print_release_output(output);
        }
        ReleaseOutcome::Executed(output) => {
            print_release_output(output);
            println!("\nRelease complete.");
        }
    }
}

fn print_release_output(output: &ReleaseOutput) {
    if output.planned_releases.is_empty() {
        println!("No packages to release.");
        return;
    }

    println!("Releases:");
    for release in &output.planned_releases {
        println!(
            "  - {} {} -> {}",
            release.name, release.current_version, release.new_version
        );
    }

    if !output.unchanged_packages.is_empty() {
        println!("\nUnchanged packages:");
        for name in &output.unchanged_packages {
            println!("  - {name}");
        }
    }

    if !output.changelog_updates.is_empty() {
        println!("\nChangelogs updated:");
        for update in &output.changelog_updates {
            let status = if update.created { "created" } else { "updated" };
            println!("  - {} ({})", update.path.display(), status);
        }
    }

    if let Some(git_result) = &output.git_result {
        print_git_result(git_result);
    }

    if !output.changesets_consumed.is_empty() {
        println!(
            "\nConsumed {} changeset file(s)",
            output.changesets_consumed.len()
        );
    }
}

fn print_git_result(git_result: &GitOperationResult) {
    if let Some(commit) = &git_result.commit {
        println!(
            "\nCommit created: {}",
            &commit.sha[..7.min(commit.sha.len())]
        );
    }

    if !git_result.tags_created.is_empty() {
        println!("\nTags created:");
        for tag in &git_result.tags_created {
            println!("  - {}", tag.name);
        }
    }

    if !git_result.changesets_deleted.is_empty() {
        println!(
            "\nDeleted {} changeset file(s)",
            git_result.changesets_deleted.len()
        );
    }
}
