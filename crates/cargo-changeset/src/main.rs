mod commands;
mod error;
mod interaction;
mod output;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

use crate::commands::Commands;
use crate::error::CliError;

#[derive(Parser)]
#[command(name = "cargo-changeset")]
#[command(bin_name = "cargo-changeset")]
#[command(about = "Manage changesets for Cargo projects", long_about = None)]
struct Cli {
    /// Path to start project discovery from (default: current directory)
    #[arg(long = "path", short = 'C', global = true)]
    path: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let start_path = match resolve_start_path(cli.path) {
        Ok(path) => path,
        Err(e) => {
            print_error(&e);
            return ExitCode::FAILURE;
        }
    };

    let (result, exec_result) = cli.command.execute(&start_path);

    if let Err(e) = result {
        if !exec_result.quiet {
            print_error(&e);
        }
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn resolve_start_path(path: Option<PathBuf>) -> Result<PathBuf, CliError> {
    match path {
        Some(p) => Ok(p),
        None => std::env::current_dir().map_err(CliError::CurrentDir),
    }
}

fn print_error(error: &CliError) {
    if let CliError::Operation(op_err) = error {
        print_operation_error(op_err);
    } else {
        eprintln!("error: {error}");

        let mut source = std::error::Error::source(error);
        while let Some(cause) = source {
            eprintln!("caused by: {cause}");
            source = std::error::Error::source(cause);
        }
    }
}

fn print_operation_error(error: &changeset_operations::OperationError) {
    use changeset_operations::OperationError;

    match error {
        OperationError::InteractionRequired => {
            eprintln!("error: interactive mode requires a terminal");
        }
        OperationError::MissingBumpType { package_name } => {
            eprintln!(
                "error: missing bump type for package '{package_name}' (use --bump or --package-bump)"
            );
        }
        OperationError::MissingDescription => {
            eprintln!("error: missing description (use -m or provide interactively)");
        }
        OperationError::EmptyDescription => {
            eprintln!("error: description cannot be empty");
        }
        OperationError::EmptyProject(path) => {
            eprintln!(
                "error: no packages found in project at '{}'",
                path.display()
            );
        }
        OperationError::UnknownPackage { name, available } => {
            eprintln!("error: unknown package '{name}' (available: {available})");
        }
        OperationError::Project(e) => {
            eprintln!("error: project error");
            eprintln!("caused by: {e}");
        }
        OperationError::Cancelled => {
            eprintln!("error: operation cancelled by user");
        }
        OperationError::SagaFailed { step, source } => {
            print_saga_failed(step, source.as_ref());
        }
        OperationError::SagaCompensationFailed {
            step,
            source,
            compensation_failures,
        } => {
            print_saga_compensation_failed(step, source.as_ref(), compensation_failures);
        }
        _ => {
            eprintln!("error: {error}");
            let mut source = std::error::Error::source(error);
            while let Some(cause) = source {
                eprintln!("caused by: {cause}");
                source = std::error::Error::source(cause);
            }
        }
    }
}

fn print_saga_failed(step: &str, source: &changeset_operations::OperationError) {
    eprintln!();
    eprintln!("Error: Release failed at step '{step}'");
    eprintln!("  -> {source}");

    let mut error_source = std::error::Error::source(source);
    while let Some(cause) = error_source {
        eprintln!("  -> {cause}");
        error_source = std::error::Error::source(cause);
    }

    eprintln!();
    eprintln!("Rollback completed successfully.");
    eprintln!("Your workspace has been restored to its original state.");
    eprintln!();
}

fn print_saga_compensation_failed(
    step: &str,
    source: &changeset_operations::OperationError,
    compensation_failures: &[changeset_operations::CompensationFailure],
) {
    eprintln!();
    eprintln!("Error: Release failed at step '{step}'");
    eprintln!("  -> {source}");

    let mut error_source = std::error::Error::source(source);
    while let Some(cause) = error_source {
        eprintln!("  -> {cause}");
        error_source = std::error::Error::source(cause);
    }

    eprintln!();
    eprintln!(
        "Rollback partially failed ({} compensation(s) failed):",
        compensation_failures.len()
    );
    eprintln!();

    for failure in compensation_failures {
        eprintln!("  x {} - {}", failure.step, failure.description);
        eprintln!("    Error: {}", failure.error);
    }

    eprintln!();
    eprintln!("WARNING: Your workspace may be in an inconsistent state.");
    eprintln!("Manual cleanup may be required.");
    eprintln!();
}
