use std::path::Path;

use changeset_operations::operations::{StatusOperation, StatusOutput};
use changeset_operations::providers::{FileSystemChangesetIO, FileSystemProjectProvider};
use changeset_operations::traits::ProjectProvider;

use crate::error::Result;

pub(crate) fn run(start_path: &Path) -> Result<()> {
    let project_provider = FileSystemProjectProvider::new();
    let project = project_provider.discover_project(start_path)?;
    let changeset_reader = FileSystemChangesetIO::new(&project.root);

    let operation = StatusOperation::new(project_provider, changeset_reader);
    let output = operation.execute(start_path)?;

    print_status(&output);

    Ok(())
}

fn print_status(output: &StatusOutput) {
    if output.changesets.is_empty() {
        println!("No pending changesets.");
        return;
    }

    println!("Pending changesets: {}", output.changesets.len());
    println!();

    if !output.projected_bumps.is_empty() {
        println!("Projected version bumps:");
        for (package, bumps) in &output.projected_bumps {
            let bump_strs: Vec<_> = bumps.iter().map(|b| format!("{b:?}")).collect();
            println!("  {package}: {}", bump_strs.join(", "));
        }
        println!();
    }

    if !output.unchanged_packages.is_empty() {
        println!("Packages without changesets:");
        for pkg in &output.unchanged_packages {
            println!("  {} ({})", pkg.name, pkg.version);
        }
    }
}
