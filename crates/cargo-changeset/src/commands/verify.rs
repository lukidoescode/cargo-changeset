use std::path::Path;

use changeset_operations::operations::{VerifyInput, VerifyOperation, VerifyOutcome};
use changeset_operations::providers::{
    FileSystemChangesetIO, FileSystemProjectProvider, Git2Provider,
};
use changeset_operations::traits::ProjectProvider;

use super::VerifyArgs;
use crate::error::{CliError, Result};
use crate::output::{OutputFormatter, PlainTextFormatter};

pub(crate) fn run(args: VerifyArgs, start_path: &Path) -> Result<()> {
    let project_provider = FileSystemProjectProvider::new();
    let project = project_provider.discover_project(start_path)?;

    let git_provider = Git2Provider::new(project.root())?;
    let changeset_reader = FileSystemChangesetIO::new(project.root());

    let operation = VerifyOperation::new(project_provider, git_provider, changeset_reader);

    let input = VerifyInput {
        base: args.base,
        head: args.head,
        allow_deleted_changesets: args.allow_deleted_changesets,
        exclude_dependents: args.exclude_dependents,
        ignore_dirty: args.ignore_dirty,
    };

    let result = operation.execute(start_path, &input)?;

    if result.is_dirty && !args.quiet {
        eprintln!("Dirty working directory detected, verifying uncommitted changes against HEAD");
    }

    let formatter = PlainTextFormatter;

    match result.outcome {
        VerifyOutcome::NoChanges => {
            if !args.quiet {
                println!("No files changed");
            }
            Ok(())
        }
        VerifyOutcome::NoPackagesAffected {
            project_file_count,
            ignored_file_count,
        } => {
            if !args.quiet {
                println!("No packages affected by changes");
                if project_file_count > 0 {
                    println!("  {project_file_count} project-level file(s) changed");
                }
                if ignored_file_count > 0 {
                    println!("  {ignored_file_count} file(s) ignored by patterns");
                }
            }
            Ok(())
        }
        VerifyOutcome::Success(verification) => {
            if !args.quiet {
                print!("{}", formatter.format_success(&verification));
            }
            Ok(())
        }
        VerifyOutcome::Failed(verification) => {
            if !args.quiet {
                eprint!("{}", formatter.format_failure(&verification));
            }
            if !verification.deleted_changesets.is_empty() {
                Err(CliError::ChangesetDeleted {
                    paths: verification.deleted_changesets,
                })
            } else {
                Err(CliError::VerificationFailed {
                    uncovered_count: verification.uncovered_packages.len(),
                })
            }
        }
    }
}
