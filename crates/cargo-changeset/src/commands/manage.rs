use std::path::Path;

use dialoguer::{Input, Select};

use changeset_core::PackageInfo;
use changeset_operations::operations::{
    GraduationDirectInput, GraduationDirectOperation, GraduationEvent, GraduationManageOperation,
    PrereleaseDirectInput, PrereleaseDirectOperation, PrereleaseEvent, PrereleaseManageOperation,
};
use changeset_operations::providers::{FileSystemProjectProvider, FileSystemReleaseStateIO};
use changeset_operations::traits::{
    GraduationAction, GraduationInteractionProvider, MenuSelection, PrereleaseAction,
    PrereleaseInteractionProvider,
};

use super::{ManageArgs, ManageCommand, ManageGraduationArgs, ManagePrereleaseArgs};
use crate::environment::is_interactive;
use crate::error::{CliError, Result};
use crate::interaction::select_from_options;
use crate::output::CliWriter;

struct TerminalManageInteractionProvider;

impl PrereleaseInteractionProvider for TerminalManageInteractionProvider {
    fn select_prerelease_action(
        &self,
    ) -> changeset_operations::Result<MenuSelection<PrereleaseAction>> {
        let options = [
            (PrereleaseAction::Add, "Add crate to pre-release"),
            (PrereleaseAction::Remove, "Remove crate from pre-release"),
            (
                PrereleaseAction::Graduate,
                "Graduate crate (move to graduation queue)",
            ),
            (PrereleaseAction::Done, "Done"),
        ];

        let selection = select_from_options("What would you like to do?", &options, 0)
            .map_err(super::cli_error_to_operation_error)?;

        Ok(MenuSelection::Selected(
            selection.unwrap_or(PrereleaseAction::Done),
        ))
    }

    fn select_package_for_prerelease(
        &self,
        available: &[&PackageInfo],
    ) -> changeset_operations::Result<MenuSelection<usize>> {
        let items: Vec<String> = available
            .iter()
            .map(|p| format!("{} ({})", p.name(), p.version()))
            .collect();

        let selection = Select::new()
            .with_prompt("Select a crate to add to pre-release")
            .items(&items)
            .interact_opt()
            .map_err(super::dialoguer_to_operation_error)?;

        Ok(match selection {
            Some(index) => MenuSelection::Selected(index),
            None => MenuSelection::Cancelled,
        })
    }

    fn get_prerelease_tag(&self) -> changeset_operations::Result<String> {
        let tag: String = Input::new()
            .with_prompt("Enter pre-release tag (e.g., alpha, beta, rc)")
            .interact_text()
            .map_err(super::dialoguer_to_operation_error)?;

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
            .map_err(super::dialoguer_to_operation_error)?;

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
            (GraduationAction::Add, "Add crate to graduation queue"),
            (
                GraduationAction::Remove,
                "Remove crate from graduation queue",
            ),
            (GraduationAction::Done, "Done"),
        ];

        let selection = select_from_options("What would you like to do?", &options, 0)
            .map_err(super::cli_error_to_operation_error)?;

        Ok(MenuSelection::Selected(
            selection.unwrap_or(GraduationAction::Done),
        ))
    }

    fn select_package_for_graduation(
        &self,
        eligible: &[&PackageInfo],
    ) -> changeset_operations::Result<MenuSelection<usize>> {
        let items: Vec<String> = eligible
            .iter()
            .map(|p| format!("{} ({})", p.name(), p.version()))
            .collect();

        let selection = Select::new()
            .with_prompt("Select a crate to graduate (move to graduation queue)")
            .items(&items)
            .interact_opt()
            .map_err(super::dialoguer_to_operation_error)?;

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
            .map_err(super::dialoguer_to_operation_error)?;

        Ok(match selection {
            Some(index) => MenuSelection::Selected(index),
            None => MenuSelection::Cancelled,
        })
    }
}

pub(super) fn run(args: ManageArgs, start_path: &Path, writer: &dyn CliWriter) -> Result<()> {
    match args.command {
        ManageCommand::Prerelease(prerelease_args) => {
            run_prerelease(prerelease_args, start_path, writer)
        }
        ManageCommand::Graduation(graduation_args) => {
            run_graduation(graduation_args, start_path, writer)
        }
    }
}

fn run_prerelease(
    args: ManagePrereleaseArgs,
    start_path: &Path,
    writer: &dyn CliWriter,
) -> Result<()> {
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

    print_prerelease_events(&events, writer);
    Ok(())
}

fn run_graduation(
    args: ManageGraduationArgs,
    start_path: &Path,
    writer: &dyn CliWriter,
) -> Result<()> {
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

    print_graduation_events(&events, writer);
    Ok(())
}

fn print_prerelease_events(events: &[PrereleaseEvent], writer: &dyn CliWriter) {
    for event in events {
        match event {
            PrereleaseEvent::DisplayState(items) => {
                writer.blank();
                if items.is_empty() {
                    writer.line("(No packages in pre-release mode)");
                } else {
                    writer.heading("Pre-release configuration (.changeset/pre-release.toml):");
                    let mut sorted = items.clone();
                    sorted.sort_by(|a, b| a.0.cmp(&b.0));
                    for (crate_name, tag) in &sorted {
                        writer.indented(&format!("{crate_name}: {tag}"));
                    }
                }
                writer.blank();
            }
            PrereleaseEvent::Added { crate_name, tag } => {
                writer.line(&format!(
                    "Added {crate_name} to pre-release configuration with tag '{tag}'"
                ));
            }
            PrereleaseEvent::Removed { crate_name } => {
                writer.line(&format!(
                    "Removed {crate_name} from pre-release configuration"
                ));
            }
            PrereleaseEvent::MovedToGraduation { crate_name } => {
                writer.line(&format!("Moved {crate_name} to graduation queue"));
            }
            PrereleaseEvent::AllPackagesInPrerelease => {
                writer.line("All packages are already in pre-release mode.");
            }
            PrereleaseEvent::NoPrereleasePackages => {
                writer.line("No packages are currently in pre-release mode.");
            }
            PrereleaseEvent::NoEligibleForGraduation => {
                writer.line(
                    "No eligible packages for graduation (must be 0.x stable version and not already queued).",
                );
            }
        }
    }
}

fn print_graduation_events(events: &[GraduationEvent], writer: &dyn CliWriter) {
    for event in events {
        match event {
            GraduationEvent::DisplayState(items) => {
                writer.blank();
                if items.is_empty() {
                    writer.line("(No packages queued for graduation)");
                } else {
                    writer.heading("Graduation queue (.changeset/graduation.toml):");
                    let mut sorted = items.clone();
                    sorted.sort();
                    for crate_name in &sorted {
                        writer.list_item(crate_name);
                    }
                }
                writer.blank();
            }
            GraduationEvent::Added { crate_name } => {
                writer.line(&format!("Added {crate_name} to graduation queue"));
            }
            GraduationEvent::Removed { crate_name } => {
                writer.line(&format!("Removed {crate_name} from graduation queue"));
            }
            GraduationEvent::NoEligibleForGraduation => {
                writer.line(
                    "No eligible packages for graduation (must be 0.x stable version and not already queued).",
                );
            }
            GraduationEvent::NoGraduationPackages => {
                writer.line("No packages are currently queued for graduation.");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use changeset_operations::operations::{GraduationEvent, PrereleaseEvent};

    use super::{print_graduation_events, print_prerelease_events};
    use crate::output::{BufferCliWriter, OutputEntry};

    #[test]
    fn prerelease_display_state_empty_shows_no_packages_message() {
        let writer = BufferCliWriter::new();
        let events = vec![PrereleaseEvent::DisplayState(vec![])];

        print_prerelease_events(&events, &writer);

        let text = writer.stdout_text();
        assert!(text.contains("(No packages in pre-release mode)"));
    }

    #[test]
    fn prerelease_display_state_with_entries_shows_sorted() {
        let writer = BufferCliWriter::new();
        let events = vec![PrereleaseEvent::DisplayState(vec![
            ("z-crate".to_string(), "beta".to_string()),
            ("a-crate".to_string(), "alpha".to_string()),
        ])];

        print_prerelease_events(&events, &writer);

        let text = writer.stdout_text();
        assert!(text.contains("Pre-release configuration"));
        let a_pos = text.find("a-crate").expect("a-crate present");
        let z_pos = text.find("z-crate").expect("z-crate present");
        assert!(a_pos < z_pos, "entries should be sorted");
    }

    #[test]
    fn prerelease_added_shows_confirmation() {
        let writer = BufferCliWriter::new();
        let events = vec![PrereleaseEvent::Added {
            crate_name: "my-crate".to_string(),
            tag: "rc".to_string(),
        }];

        print_prerelease_events(&events, &writer);

        let text = writer.stdout_text();
        assert!(text.contains("Added my-crate to pre-release configuration with tag 'rc'"));
    }

    #[test]
    fn graduation_display_state_empty() {
        let writer = BufferCliWriter::new();
        let events = vec![GraduationEvent::DisplayState(vec![])];

        print_graduation_events(&events, &writer);

        let text = writer.stdout_text();
        assert!(text.contains("(No packages queued for graduation)"));
    }

    #[test]
    fn graduation_display_state_with_entries_shows_sorted_list_items() {
        let writer = BufferCliWriter::new();
        let events = vec![GraduationEvent::DisplayState(vec![
            "z-pkg".to_string(),
            "a-pkg".to_string(),
        ])];

        print_graduation_events(&events, &writer);

        let entries = writer.stdout_entries();
        assert!(entries.contains(&OutputEntry::ListItem("a-pkg".to_string())));
        assert!(entries.contains(&OutputEntry::ListItem("z-pkg".to_string())));
    }

    #[test]
    fn graduation_added_shows_confirmation() {
        let writer = BufferCliWriter::new();
        let events = vec![GraduationEvent::Added {
            crate_name: "my-lib".to_string(),
        }];

        print_graduation_events(&events, &writer);

        let text = writer.stdout_text();
        assert!(text.contains("Added my-lib to graduation queue"));
    }
}
