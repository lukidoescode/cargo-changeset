use std::path::Path;

use changeset_operations::operations::{
    ReleaseInput, ReleaseOperation, ReleaseOutcome, ReleaseOutput,
};
use changeset_operations::providers::{
    FileSystemChangesetIO, FileSystemManifestWriter, FileSystemProjectProvider,
};
use changeset_operations::traits::ProjectProvider;

use super::ReleaseArgs;
use crate::error::Result;

pub(crate) fn run(args: ReleaseArgs, start_path: &Path) -> Result<()> {
    let project_provider = FileSystemProjectProvider::new();
    let project = project_provider.discover_project(start_path)?;
    let changeset_reader = FileSystemChangesetIO::new(&project.root);
    let manifest_writer = FileSystemManifestWriter::new();

    let operation = ReleaseOperation::new(project_provider, changeset_reader, manifest_writer);
    let input = ReleaseInput {
        dry_run: args.dry_run,
        convert_inherited: args.convert,
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

    println!("Planned releases:");
    for release in &output.planned_releases {
        println!(
            "  {} {} -> {} ({:?})",
            release.name, release.current_version, release.new_version, release.bump_type
        );
    }

    if !output.unchanged_packages.is_empty() {
        println!("\nUnchanged packages:");
        for name in &output.unchanged_packages {
            println!("  {name}");
        }
    }

    println!(
        "\nChangesets to consume: {}",
        output.changesets_consumed.len()
    );
}
