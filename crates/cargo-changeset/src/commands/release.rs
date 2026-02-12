use std::path::Path;

use changeset_operations::operations::{
    GitOperationResult, ReleaseInput, ReleaseOperation, ReleaseOutcome, ReleaseOutput,
};
use changeset_operations::providers::{
    FileSystemChangelogWriter, FileSystemChangesetIO, FileSystemManifestWriter,
    FileSystemProjectProvider, Git2Provider,
};
use changeset_operations::traits::ProjectProvider;

use super::ReleaseArgs;
use crate::error::Result;

pub(crate) fn run(args: ReleaseArgs, start_path: &Path) -> Result<()> {
    let project_provider = FileSystemProjectProvider::new();
    let project = project_provider.discover_project(start_path)?;
    let changeset_reader = FileSystemChangesetIO::new(&project.root);
    let manifest_writer = FileSystemManifestWriter::new();
    let changelog_writer = FileSystemChangelogWriter::new();
    let git_provider = Git2Provider::new();

    let operation = ReleaseOperation::new(
        project_provider,
        changeset_reader,
        manifest_writer,
        changelog_writer,
        git_provider,
    );
    let input = ReleaseInput {
        dry_run: args.dry_run,
        convert_inherited: args.convert,
        no_commit: args.no_commit,
        no_tags: args.no_tags,
        keep_changesets: args.keep_changesets,
    };
    let outcome = operation.execute(start_path, &input)?;

    print_outcome(&outcome);

    Ok(())
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
