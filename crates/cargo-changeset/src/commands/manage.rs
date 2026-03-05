use std::path::Path;

use changeset_core::PackageInfo;
use changeset_operations::OperationError;
use changeset_operations::operations::{
    GraduationDirectInput, GraduationDirectOperation, GraduationEvent, GraduationManageOperation,
    PrereleaseDirectInput, PrereleaseDirectOperation, PrereleaseEvent, PrereleaseManageOperation,
};
use changeset_operations::providers::{FileSystemProjectProvider, FileSystemReleaseStateIO};
use changeset_operations::traits::{
    GraduationAction, GraduationInteractionProvider, MenuSelection, PrereleaseAction,
    PrereleaseInteractionProvider,
};
use dialoguer::{Input, Select};

use super::{ManageArgs, ManageCommand, ManageGraduationArgs, ManagePrereleaseArgs};
use crate::environment::is_interactive;
use crate::error::{CliError, Result};

pub(crate) fn run(args: ManageArgs, start_path: &Path) -> Result<()> {
    match args.command {
        ManageCommand::Prerelease(prerelease_args) => run_prerelease(prerelease_args, start_path),
        ManageCommand::Graduation(graduation_args) => run_graduation(graduation_args, start_path),
    }
}

fn run_prerelease(args: ManagePrereleaseArgs, start_path: &Path) -> Result<()> {
    let no_flags_provided =
        args.add.is_empty() && args.remove.is_empty() && args.graduate.is_empty() && !args.list;

    let events = if no_flags_provided {
        if !is_interactive() {
            return Err(CliError::NotATty);
        }

        let interaction = TerminalManageInteractionProvider;
        let operation = PrereleaseManageOperation::new(
            FileSystemProjectProvider::new(),
            FileSystemReleaseStateIO::new(),
            interaction,
        );
        operation.execute(start_path)?
    } else {
        let operation = PrereleaseDirectOperation::new(
            FileSystemProjectProvider::new(),
            FileSystemReleaseStateIO::new(),
        );
        let input = PrereleaseDirectInput::new(args.add, args.remove, args.graduate, args.list);
        operation.execute(start_path, &input)?
    };

    print_prerelease_events(&events);
    Ok(())
}

fn run_graduation(args: ManageGraduationArgs, start_path: &Path) -> Result<()> {
    let no_flags_provided = args.add.is_empty() && args.remove.is_empty() && !args.list;

    let events = if no_flags_provided {
        if !is_interactive() {
            return Err(CliError::NotATty);
        }

        let interaction = TerminalManageInteractionProvider;
        let operation = GraduationManageOperation::new(
            FileSystemProjectProvider::new(),
            FileSystemReleaseStateIO::new(),
            interaction,
        );
        operation.execute(start_path)?
    } else {
        let operation = GraduationDirectOperation::new(
            FileSystemProjectProvider::new(),
            FileSystemReleaseStateIO::new(),
        );
        let input = GraduationDirectInput::new(args.add, args.remove, args.list);
        operation.execute(start_path, &input)?
    };

    print_graduation_events(&events);
    Ok(())
}

fn print_prerelease_events(events: &[PrereleaseEvent]) {
    for event in events {
        match event {
            PrereleaseEvent::DisplayState(items) => {
                println!();
                if items.is_empty() {
                    println!("(No packages in pre-release mode)");
                } else {
                    println!("Pre-release configuration (.changeset/pre-release.toml):");
                    let mut sorted = items.clone();
                    sorted.sort_by(|a, b| a.0.cmp(&b.0));
                    for (crate_name, tag) in &sorted {
                        println!("  {crate_name}: {tag}");
                    }
                }
                println!();
            }
            PrereleaseEvent::Added { crate_name, tag } => {
                println!("Added {crate_name} to pre-release configuration with tag '{tag}'");
            }
            PrereleaseEvent::Removed { crate_name } => {
                println!("Removed {crate_name} from pre-release configuration");
            }
            PrereleaseEvent::MovedToGraduation { crate_name } => {
                println!("Moved {crate_name} to graduation queue");
            }
            PrereleaseEvent::AllPackagesInPrerelease => {
                println!("All packages are already in pre-release mode.");
            }
            PrereleaseEvent::NoPrereleasePackages => {
                println!("No packages are currently in pre-release mode.");
            }
            PrereleaseEvent::NoEligibleForGraduation => {
                println!(
                    "No eligible packages for graduation (must be 0.x stable version and not already queued)."
                );
            }
        }
    }
}

fn print_graduation_events(events: &[GraduationEvent]) {
    for event in events {
        match event {
            GraduationEvent::DisplayState(items) => {
                println!();
                if items.is_empty() {
                    println!("(No packages queued for graduation)");
                } else {
                    println!("Graduation queue (.changeset/graduation.toml):");
                    let mut sorted = items.clone();
                    sorted.sort();
                    for crate_name in &sorted {
                        println!("  - {crate_name}");
                    }
                }
                println!();
            }
            GraduationEvent::Added { crate_name } => {
                println!("Added {crate_name} to graduation queue");
            }
            GraduationEvent::Removed { crate_name } => {
                println!("Removed {crate_name} from graduation queue");
            }
            GraduationEvent::NoEligibleForGraduation => {
                println!(
                    "No eligible packages for graduation (must be 0.x stable version and not already queued)."
                );
            }
            GraduationEvent::NoGraduationPackages => {
                println!("No packages are currently queued for graduation.");
            }
        }
    }
}

struct TerminalManageInteractionProvider;

impl PrereleaseInteractionProvider for TerminalManageInteractionProvider {
    fn select_prerelease_action(
        &self,
    ) -> changeset_operations::Result<MenuSelection<PrereleaseAction>> {
        let options = [
            "Add crate to pre-release",
            "Remove crate from pre-release",
            "Graduate crate (move to graduation queue)",
            "Done",
        ];

        let selection = Select::new()
            .with_prompt("What would you like to do?")
            .items(options)
            .default(0)
            .interact_opt()
            .map_err(dialoguer_to_operation_error)?;

        Ok(match selection {
            Some(0) => MenuSelection::Selected(PrereleaseAction::Add),
            Some(1) => MenuSelection::Selected(PrereleaseAction::Remove),
            Some(2) => MenuSelection::Selected(PrereleaseAction::Graduate),
            _ => MenuSelection::Selected(PrereleaseAction::Done),
        })
    }

    fn select_package_for_prerelease(
        &self,
        available: &[&PackageInfo],
    ) -> changeset_operations::Result<MenuSelection<usize>> {
        let items: Vec<String> = available
            .iter()
            .map(|p| format!("{} ({})", p.name, p.version))
            .collect();

        let selection = Select::new()
            .with_prompt("Select a crate to add to pre-release")
            .items(&items)
            .interact_opt()
            .map_err(dialoguer_to_operation_error)?;

        Ok(match selection {
            Some(index) => MenuSelection::Selected(index),
            None => MenuSelection::Cancelled,
        })
    }

    fn get_prerelease_tag(&self) -> changeset_operations::Result<String> {
        let tag: String = Input::new()
            .with_prompt("Enter pre-release tag (e.g., alpha, beta, rc)")
            .interact_text()
            .map_err(dialoguer_to_operation_error)?;

        Ok(tag)
    }

    fn select_package_to_remove_prerelease(
        &self,
        items: &[(&str, &str)],
    ) -> changeset_operations::Result<MenuSelection<usize>> {
        let display_items: Vec<String> = items
            .iter()
            .map(|(name, tag)| format!("{name}: {tag}"))
            .collect();

        let selection = Select::new()
            .with_prompt("Select a crate to remove from pre-release")
            .items(&display_items)
            .interact_opt()
            .map_err(dialoguer_to_operation_error)?;

        Ok(match selection {
            Some(index) => MenuSelection::Selected(index),
            None => MenuSelection::Cancelled,
        })
    }
}

impl GraduationInteractionProvider for TerminalManageInteractionProvider {
    fn select_graduation_action(
        &self,
    ) -> changeset_operations::Result<MenuSelection<GraduationAction>> {
        let options = [
            "Add crate to graduation queue",
            "Remove crate from graduation queue",
            "Done",
        ];

        let selection = Select::new()
            .with_prompt("What would you like to do?")
            .items(options)
            .default(0)
            .interact_opt()
            .map_err(dialoguer_to_operation_error)?;

        Ok(match selection {
            Some(0) => MenuSelection::Selected(GraduationAction::Add),
            Some(1) => MenuSelection::Selected(GraduationAction::Remove),
            _ => MenuSelection::Selected(GraduationAction::Done),
        })
    }

    fn select_package_for_graduation(
        &self,
        eligible: &[&PackageInfo],
    ) -> changeset_operations::Result<MenuSelection<usize>> {
        let items: Vec<String> = eligible
            .iter()
            .map(|p| format!("{} ({})", p.name, p.version))
            .collect();

        let selection = Select::new()
            .with_prompt("Select a crate to graduate (move to graduation queue)")
            .items(&items)
            .interact_opt()
            .map_err(dialoguer_to_operation_error)?;

        Ok(match selection {
            Some(index) => MenuSelection::Selected(index),
            None => MenuSelection::Cancelled,
        })
    }

    fn select_package_to_remove_graduation(
        &self,
        items: &[String],
    ) -> changeset_operations::Result<MenuSelection<usize>> {
        let selection = Select::new()
            .with_prompt("Select a crate to remove from graduation queue")
            .items(items)
            .interact_opt()
            .map_err(dialoguer_to_operation_error)?;

        Ok(match selection {
            Some(index) => MenuSelection::Selected(index),
            None => MenuSelection::Cancelled,
        })
    }
}

fn dialoguer_to_operation_error(e: dialoguer::Error) -> OperationError {
    match e {
        dialoguer::Error::IO(io_err) => OperationError::Io(io_err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod dialoguer_conversion {
        use super::*;
        use std::io;

        #[test]
        fn converts_io_error_to_operation_io_variant() {
            let io_err = io::Error::new(io::ErrorKind::BrokenPipe, "pipe closed");
            let dialoguer_err = dialoguer::Error::IO(io_err);

            let result = dialoguer_to_operation_error(dialoguer_err);

            assert!(matches!(result, OperationError::Io(_)));
        }

        #[test]
        fn preserves_io_error_kind() {
            let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "access denied");
            let dialoguer_err = dialoguer::Error::IO(io_err);

            let result = dialoguer_to_operation_error(dialoguer_err);

            match result {
                OperationError::Io(inner) => {
                    assert_eq!(inner.kind(), io::ErrorKind::PermissionDenied);
                }
                other => panic!("expected OperationError::Io, got {other:?}"),
            }
        }

        #[test]
        fn preserves_io_error_message() {
            let io_err = io::Error::other("terminal unavailable");
            let dialoguer_err = dialoguer::Error::IO(io_err);

            let result = dialoguer_to_operation_error(dialoguer_err);

            assert!(
                result.to_string().contains("IO error"),
                "expected display to contain 'IO error', got: {}",
                result
            );
            match result {
                OperationError::Io(inner) => {
                    assert_eq!(inner.to_string(), "terminal unavailable");
                }
                other => panic!("expected OperationError::Io, got {other:?}"),
            }
        }
    }
}
