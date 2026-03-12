use std::fmt::Write;

use crate::environment::NonInteractiveReason;
use crate::error::CliError;

pub(crate) fn print_error(error: &CliError) {
    let formatted = format_error(error);
    eprint!("{formatted}");
}

fn format_error(error: &CliError) -> String {
    let mut out = String::new();
    if let CliError::Operation(op_err) = error {
        format_operation_error(&mut out, op_err);
    } else {
        writeln!(out, "error: {error}").expect("string write");

        let mut source = std::error::Error::source(error);
        while let Some(cause) = source {
            writeln!(out, "caused by: {cause}").expect("string write");
            source = std::error::Error::source(cause);
        }
    }
    out
}

fn format_operation_error(out: &mut String, error: &changeset_operations::OperationError) {
    use changeset_operations::OperationError;

    match error {
        OperationError::InteractionRequired => match crate::environment::non_interactive_reason() {
            Some(NonInteractiveReason::CiDetected { env_var }) => {
                writeln!(
                    out,
                    "error: interactive input required but running in CI environment \
                         (detected via ${env_var})"
                )
                .expect("string write");
                writeln!(out).expect("string write");
                writeln!(out, "To use this command non-interactively, provide:")
                    .expect("string write");
                writeln!(
                    out,
                    "  --package <PACKAGE>    Specify package(s) to include"
                )
                .expect("string write");
                writeln!(
                    out,
                    "  --bump <TYPE>          Bump type: major, minor, patch, or none"
                )
                .expect("string write");
                writeln!(out, "  -m <MESSAGE>           Change description").expect("string write");
                writeln!(out).expect("string write");
                writeln!(out, "Example:").expect("string write");
                writeln!(
                    out,
                    "  cargo changeset add --package my-crate --bump minor -m \"Added feature\""
                )
                .expect("string write");
            }
            Some(NonInteractiveReason::ExplicitDisable) => {
                writeln!(
                    out,
                    "error: interactive mode disabled via CARGO_CHANGESET_NO_TTY"
                )
                .expect("string write");
            }
            Some(NonInteractiveReason::NoTerminal) | None => {
                writeln!(out, "error: interactive mode requires a terminal").expect("string write");
            }
        },
        OperationError::MissingBumpType { package_name } => {
            writeln!(
                out,
                "error: missing bump type for package '{package_name}' (use --bump or --package-bump)"
            )
            .expect("string write");
        }
        OperationError::MissingDescription => {
            writeln!(
                out,
                "error: missing description (use -m or provide interactively)"
            )
            .expect("string write");
        }
        OperationError::EmptyDescription => {
            writeln!(out, "error: description cannot be empty").expect("string write");
        }
        OperationError::EmptyProject(path) => {
            writeln!(
                out,
                "error: no packages found in project at '{}'",
                path.display()
            )
            .expect("string write");
        }
        OperationError::UnknownPackage { name, available } => {
            writeln!(
                out,
                "error: unknown package '{name}' (available: {available})"
            )
            .expect("string write");
        }
        OperationError::Project(e) => {
            writeln!(out, "error: project error").expect("string write");
            writeln!(out, "caused by: {e}").expect("string write");
        }
        OperationError::Cancelled => {
            writeln!(out, "error: operation cancelled by user").expect("string write");
        }
        OperationError::SagaFailed { step, source } => {
            format_saga_failed(out, step, source.as_ref());
        }
        OperationError::SagaCompensationFailed {
            step,
            source,
            compensation_failures,
        } => {
            format_saga_compensation_failed(out, step, source.as_ref(), compensation_failures);
        }
        _ => {
            writeln!(out, "error: {error}").expect("string write");
            let mut source = std::error::Error::source(error);
            while let Some(cause) = source {
                writeln!(out, "caused by: {cause}").expect("string write");
                source = std::error::Error::source(cause);
            }
        }
    }
}

fn format_saga_failed(out: &mut String, step: &str, source: &changeset_operations::OperationError) {
    writeln!(out).expect("string write");
    writeln!(out, "Error: Release failed at step '{step}'").expect("string write");
    writeln!(out, "  -> {source}").expect("string write");

    let mut error_source = std::error::Error::source(source);
    while let Some(cause) = error_source {
        writeln!(out, "  -> {cause}").expect("string write");
        error_source = std::error::Error::source(cause);
    }

    writeln!(out).expect("string write");
    writeln!(out, "Rollback completed successfully.").expect("string write");
    writeln!(
        out,
        "Your workspace has been restored to its original state."
    )
    .expect("string write");
    writeln!(out).expect("string write");
}

fn format_saga_compensation_failed(
    out: &mut String,
    step: &str,
    source: &changeset_operations::OperationError,
    compensation_failures: &[changeset_operations::CompensationFailure],
) {
    writeln!(out).expect("string write");
    writeln!(out, "Error: Release failed at step '{step}'").expect("string write");
    writeln!(out, "  -> {source}").expect("string write");

    let mut error_source = std::error::Error::source(source);
    while let Some(cause) = error_source {
        writeln!(out, "  -> {cause}").expect("string write");
        error_source = std::error::Error::source(cause);
    }

    writeln!(out).expect("string write");
    writeln!(
        out,
        "Rollback partially failed ({} compensation(s) failed):",
        compensation_failures.len()
    )
    .expect("string write");
    writeln!(out).expect("string write");

    for failure in compensation_failures {
        writeln!(out, "  x {} - {}", failure.step, failure.description).expect("string write");
        writeln!(out, "    Error: {}", failure.error).expect("string write");
    }

    writeln!(out).expect("string write");
    writeln!(
        out,
        "WARNING: Your workspace may be in an inconsistent state."
    )
    .expect("string write");
    writeln!(out, "Manual cleanup may be required.").expect("string write");
    writeln!(out).expect("string write");
}

#[cfg(test)]
mod tests {
    use super::*;
    use changeset_operations::{CompensationFailure, OperationError};
    use std::path::PathBuf;

    #[test]
    fn format_saga_failed_includes_step_and_rollback_message() {
        let error = CliError::Operation(OperationError::SagaFailed {
            step: "write-manifests".to_string(),
            source: Box::new(OperationError::Cancelled),
        });

        let output = format_error(&error);

        assert!(output.contains("write-manifests"));
        assert!(output.contains("Rollback completed successfully"));
        assert!(output.contains("restored to its original state"));
    }

    #[test]
    fn format_saga_compensation_failed_includes_failures() {
        let error = CliError::Operation(OperationError::SagaCompensationFailed {
            step: "create-tags".to_string(),
            source: Box::new(OperationError::Cancelled),
            compensation_failures: vec![CompensationFailure {
                step: "write-manifests".to_string(),
                description: "restore Cargo.toml files".to_string(),
                error: Box::new(OperationError::Io(std::io::Error::other("disk full"))),
            }],
        });

        let output = format_error(&error);

        assert!(output.contains("create-tags"));
        assert!(output.contains("Rollback partially failed"));
        assert!(output.contains("1 compensation(s) failed"));
        assert!(output.contains("write-manifests"));
        assert!(output.contains("restore Cargo.toml files"));
        assert!(output.contains("WARNING"));
        assert!(output.contains("inconsistent state"));
    }

    #[test]
    fn format_simple_error_variant() {
        let error =
            CliError::Operation(OperationError::EmptyProject(PathBuf::from("/my/workspace")));

        let output = format_error(&error);

        assert!(output.contains("/my/workspace"));
        assert!(output.contains("no packages found"));
    }

    #[test]
    fn format_non_operation_error_uses_generic_path() {
        let error = CliError::NotATty;

        let output = format_error(&error);

        assert!(output.contains("error:"));
        assert!(output.contains("terminal"));
    }

    #[test]
    fn format_cancelled_error() {
        let error = CliError::Operation(OperationError::Cancelled);

        let output = format_error(&error);

        assert!(output.contains("cancelled by user"));
    }
}
