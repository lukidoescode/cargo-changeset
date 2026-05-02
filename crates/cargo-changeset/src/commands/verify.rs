use std::path::Path;

use changeset_operations::operations::{VerifyInputBuilder, VerifyOperation, VerifyOutcome};
use changeset_operations::providers::{
    FileSystemChangesetIO, FileSystemProjectProvider, Git2Provider,
};
use changeset_operations::traits::ProjectProvider;

use super::VerifyArgs;
use crate::error::{CliError, Result};
use crate::output::{CliWriter, OutputFormatter, PlainTextFormatter};

pub(super) fn run(args: VerifyArgs, start_path: &Path, writer: &dyn CliWriter) -> Result<()> {
    let project_provider = FileSystemProjectProvider::new();
    let project = project_provider.discover_project(start_path)?;
    let (root_config, _) = project_provider.load_configs(&project)?;

    let git_provider = Git2Provider::new(project.root())?;
    let changeset_reader = FileSystemChangesetIO::new(project.root());

    let operation = VerifyOperation::new(project_provider, git_provider, changeset_reader);

    let base = args
        .base
        .unwrap_or_else(|| root_config.base_branch().to_string());

    let input = VerifyInputBuilder::default()
        .base(base)
        .head(args.head)
        .allow_deleted_changesets(args.allow_deleted_changesets)
        .exclude_dependents(args.exclude_dependents)
        .ignore_dirty(args.ignore_dirty)
        .build()
        .expect("all fields have defaults");

    let result = operation.execute(start_path, &input)?;

    if result.is_dirty() && !args.quiet {
        writer.warn_stderr(
            "Dirty working directory detected, verifying uncommitted changes against HEAD",
        );
    }

    print_outcome(result.outcome(), args.quiet, writer)
}

fn print_outcome(outcome: &VerifyOutcome, quiet: bool, writer: &dyn CliWriter) -> Result<()> {
    let formatter = PlainTextFormatter;

    match outcome {
        VerifyOutcome::NoChanges => {
            if !quiet {
                writer.line("No files changed");
            }
            Ok(())
        }
        VerifyOutcome::NoPackagesAffected {
            project_file_count,
            ignored_file_count,
        } => {
            if !quiet {
                writer.line("No packages affected by changes");
                if *project_file_count > 0 {
                    writer.indented(&format!(
                        "{project_file_count} project-level file(s) changed"
                    ));
                }
                if *ignored_file_count > 0 {
                    writer.indented(&format!("{ignored_file_count} file(s) ignored by patterns"));
                }
            }
            Ok(())
        }
        VerifyOutcome::Success(verification) => {
            if !quiet {
                writer.raw(&formatter.format_success(verification));
            }
            Ok(())
        }
        VerifyOutcome::Failed(verification) => {
            if !quiet {
                writer.raw_stderr(&formatter.format_failure(verification));
            }
            if !verification.deleted_changesets().is_empty() {
                Err(CliError::ChangesetDeleted {
                    paths: verification.deleted_changesets().clone(),
                })
            } else {
                Err(CliError::VerificationFailed {
                    uncovered_count: verification.uncovered_packages().len(),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::PathBuf;

    use changeset_core::PackageInfo;
    use changeset_operations::operations::VerifyOutcome;
    use changeset_operations::verification::VerificationResult;
    use semver::Version;

    use super::print_outcome;
    use crate::error::CliError;
    use crate::output::BufferCliWriter;

    fn empty_verification() -> VerificationResult {
        VerificationResult::new(vec![], HashSet::new(), vec![], vec![])
    }

    fn package_info(name: &str) -> PackageInfo {
        PackageInfo::new(
            name.to_string(),
            Version::new(1, 0, 0),
            PathBuf::from(format!("crates/{name}")),
        )
    }

    #[test]
    fn no_changes_prints_message() {
        let writer = BufferCliWriter::new();

        let result = print_outcome(&VerifyOutcome::NoChanges, false, &writer);

        assert!(result.is_ok());
        let text = writer.stdout_text();
        assert!(text.contains("No files changed"));
    }

    #[test]
    fn no_changes_quiet_suppresses_output() {
        let writer = BufferCliWriter::new();

        let result = print_outcome(&VerifyOutcome::NoChanges, true, &writer);

        assert!(result.is_ok());
        assert!(writer.stdout_entries().is_empty());
    }

    #[test]
    fn no_packages_affected_shows_counts() {
        let writer = BufferCliWriter::new();
        let outcome = VerifyOutcome::NoPackagesAffected {
            project_file_count: 3,
            ignored_file_count: 2,
        };

        let result = print_outcome(&outcome, false, &writer);

        assert!(result.is_ok());
        let text = writer.stdout_text();
        assert!(text.contains("No packages affected by changes"));
        assert!(text.contains("3 project-level file(s) changed"));
        assert!(text.contains("2 file(s) ignored by patterns"));
    }

    #[test]
    fn no_packages_affected_hides_zero_counts() {
        let writer = BufferCliWriter::new();
        let outcome = VerifyOutcome::NoPackagesAffected {
            project_file_count: 0,
            ignored_file_count: 0,
        };

        let result = print_outcome(&outcome, false, &writer);

        assert!(result.is_ok());
        let text = writer.stdout_text();
        assert!(text.contains("No packages affected by changes"));
        assert!(!text.contains("project-level"));
        assert!(!text.contains("ignored"));
    }

    #[test]
    fn success_renders_via_formatter() {
        let writer = BufferCliWriter::new();
        let verification = empty_verification();

        let result = print_outcome(&VerifyOutcome::Success(verification), false, &writer);

        assert!(result.is_ok());
        let text = writer.stdout_text();
        assert!(text.contains("All changed packages have changeset coverage"));
    }

    #[test]
    fn success_quiet_suppresses_output() {
        let writer = BufferCliWriter::new();
        let verification = empty_verification();

        let result = print_outcome(&VerifyOutcome::Success(verification), true, &writer);

        assert!(result.is_ok());
        assert!(writer.stdout_entries().is_empty());
    }

    #[test]
    fn failed_with_uncovered_returns_verification_error() {
        let writer = BufferCliWriter::new();
        let mut verification = VerificationResult::new(
            vec![package_info("my-crate")],
            HashSet::new(),
            vec![],
            vec![],
        );
        verification.set_uncovered_packages(vec![package_info("my-crate")]);

        let result = print_outcome(&VerifyOutcome::Failed(verification), false, &writer);

        assert!(result.is_err());
        match result.unwrap_err() {
            CliError::VerificationFailed { uncovered_count } => {
                assert_eq!(uncovered_count, 1);
            }
            other => panic!("expected VerificationFailed, got {other:?}"),
        }
    }

    #[test]
    fn failed_with_deleted_changesets_returns_deleted_error() {
        let writer = BufferCliWriter::new();
        let mut verification = empty_verification();
        verification.set_deleted_changesets(vec![PathBuf::from(".changeset/old.md")]);

        let result = print_outcome(&VerifyOutcome::Failed(verification), false, &writer);

        assert!(result.is_err());
        match result.unwrap_err() {
            CliError::ChangesetDeleted { paths } => {
                assert_eq!(paths.len(), 1);
            }
            other => panic!("expected ChangesetDeleted, got {other:?}"),
        }
    }

    #[test]
    fn failed_quiet_suppresses_output_but_still_errors() {
        let writer = BufferCliWriter::new();
        let mut verification = empty_verification();
        verification.set_uncovered_packages(vec![package_info("my-crate")]);

        let result = print_outcome(&VerifyOutcome::Failed(verification), true, &writer);

        assert!(result.is_err());
        assert!(writer.stdout_entries().is_empty());
        assert!(writer.stderr_entries().is_empty());
    }
}
