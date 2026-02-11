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

    let git_provider = Git2Provider::new();
    let changeset_reader = FileSystemChangesetIO::new(&project.root);

    let operation = VerifyOperation::new(project_provider, git_provider, changeset_reader);

    let input = VerifyInput {
        base: args.base,
        head: args.head,
        allow_deleted_changesets: args.allow_deleted_changesets,
    };

    let outcome = operation.execute(start_path, &input)?;

    let formatter = PlainTextFormatter;

    match outcome {
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
        VerifyOutcome::Success(result) => {
            if !args.quiet {
                print!("{}", formatter.format_success(&result));
            }
            Ok(())
        }
        VerifyOutcome::Failed(result) => {
            if !args.quiet {
                eprint!("{}", formatter.format_failure(&result));
            }
            if !result.deleted_changesets.is_empty() {
                Err(CliError::ChangesetDeleted {
                    paths: result.deleted_changesets,
                })
            } else {
                Err(CliError::VerificationFailed {
                    uncovered_count: result.uncovered_packages.len(),
                })
            }
        }
    }
}
