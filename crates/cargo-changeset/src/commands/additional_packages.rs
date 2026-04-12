use std::path::{Path, PathBuf};

use clap::{Args, Subcommand, ValueEnum};
use dialoguer::{Confirm, Input, Select};

use changeset_core::{AdditionalPackageDeclaration, ManifestFormat};
use changeset_manifest::AdditionalPackageUpdate;
use changeset_operations::OperationError;
use changeset_operations::operations::{
    AdditionalPackageAddInput, AdditionalPackageDirectAddOperation,
    AdditionalPackageDirectEditOperation, AdditionalPackageDirectRemoveOperation,
    AdditionalPackageEditInput, AdditionalPackageEvent, AdditionalPackageInteractiveAddOperation,
    AdditionalPackageInteractiveEditOperation, AdditionalPackageInteractiveRemoveOperation,
    AdditionalPackageListOperation,
};
use changeset_operations::providers::{FileSystemManifestWriter, FileSystemProjectProvider};
use changeset_operations::traits::{
    AdditionalPackageField, AdditionalPackageInteractionProvider, MenuSelection,
};

use crate::environment::is_interactive;
use crate::error::{CliError, Result};

#[derive(Args)]
pub(crate) struct AdditionalPackagesArgs {
    #[command(subcommand)]
    pub(crate) command: AdditionalPackageCommand,
}

#[derive(Subcommand)]
pub(crate) enum AdditionalPackageCommand {
    /// Add a non-Rust package declaration
    Add(AddAdditionalPackageArgs),
    /// Remove a non-Rust package declaration
    Remove(RemoveAdditionalPackageArgs),
    /// Edit a non-Rust package declaration
    Edit(EditAdditionalPackageArgs),
    /// List all non-Rust package declarations
    List,
}

#[derive(Args)]
pub(crate) struct AddAdditionalPackageArgs {
    /// Package name
    #[arg(long)]
    pub name: Option<String>,

    /// Package directory path (relative to workspace root)
    #[arg(long)]
    pub path: Option<PathBuf>,

    /// Glob patterns that influence this package (can be repeated)
    #[arg(long = "influence", value_name = "GLOB")]
    pub influence: Vec<String>,

    /// Path to the version manifest file
    #[arg(long = "manifest-file")]
    pub manifest_file: Option<PathBuf>,

    /// Format of the version manifest
    #[arg(long = "manifest-format", value_enum)]
    pub manifest_format: Option<ManifestFormat>,

    /// JSONPath-style path to the version field in the manifest
    #[arg(long = "version-path")]
    pub version_path: Option<String>,
}

#[derive(Args)]
pub(crate) struct RemoveAdditionalPackageArgs {
    /// Package name to remove
    #[arg(long)]
    pub name: Option<String>,
}

#[derive(Args)]
pub(crate) struct EditAdditionalPackageArgs {
    /// Package name to edit
    #[arg(long)]
    pub name: Option<String>,

    /// New package directory path
    #[arg(long)]
    pub path: Option<PathBuf>,

    /// New influence glob patterns (replaces all existing patterns)
    #[arg(long = "influence", value_name = "GLOB")]
    pub influence: Vec<String>,

    /// New manifest file path
    #[arg(long = "manifest-file")]
    pub manifest_file: Option<PathBuf>,

    /// New manifest format
    #[arg(long = "manifest-format", value_enum)]
    pub manifest_format: Option<ManifestFormat>,

    /// New version path
    #[arg(long = "version-path")]
    pub version_path: Option<String>,
}

struct TerminalAdditionalPackageInteractionProvider;

impl AdditionalPackageInteractionProvider for TerminalAdditionalPackageInteractionProvider {
    fn prompt_package_name(&self) -> changeset_operations::Result<String> {
        Input::new()
            .with_prompt("Package name")
            .interact_text()
            .map_err(dialoguer_to_operation_error)
    }

    fn prompt_package_path(&self) -> changeset_operations::Result<PathBuf> {
        let s: String = Input::new()
            .with_prompt("Package directory path (relative to workspace root)")
            .interact_text()
            .map_err(dialoguer_to_operation_error)?;
        Ok(PathBuf::from(s))
    }

    fn prompt_influence_patterns(&self) -> changeset_operations::Result<Vec<String>> {
        let mut patterns = Vec::new();
        println!(
            "Enter glob patterns for files that influence this package (empty line to finish):"
        );
        loop {
            let s: String = Input::new()
                .with_prompt("Glob pattern")
                .allow_empty(true)
                .interact_text()
                .map_err(dialoguer_to_operation_error)?;
            if s.is_empty() {
                break;
            }
            patterns.push(s);
        }
        Ok(patterns)
    }

    fn prompt_manifest_file_path(&self) -> changeset_operations::Result<PathBuf> {
        let s: String = Input::new()
            .with_prompt("Path to version manifest file")
            .interact_text()
            .map_err(dialoguer_to_operation_error)?;
        Ok(PathBuf::from(s))
    }

    fn prompt_manifest_format(&self) -> changeset_operations::Result<ManifestFormat> {
        let variants = ManifestFormat::value_variants();
        let items: Vec<String> = variants.iter().map(ToString::to_string).collect();
        let selection = Select::new()
            .with_prompt("Manifest format")
            .items(&items)
            .default(0)
            .interact_opt()
            .map_err(dialoguer_to_operation_error)?;

        match selection {
            Some(index) => Ok(variants[index]),
            None => Err(OperationError::Cancelled),
        }
    }

    fn prompt_manifest_version_path(&self) -> changeset_operations::Result<String> {
        Input::new()
            .with_prompt("Path to version field in manifest (e.g. 'version' or 'info.version')")
            .interact_text()
            .map_err(dialoguer_to_operation_error)
    }

    fn select_package_to_remove(
        &self,
        packages: &[&AdditionalPackageDeclaration],
    ) -> changeset_operations::Result<MenuSelection<usize>> {
        let items: Vec<String> = packages
            .iter()
            .map(|p| format!("{} ({})", p.name(), p.path().display()))
            .collect();

        let selection = Select::new()
            .with_prompt("Select package to remove")
            .items(&items)
            .interact_opt()
            .map_err(dialoguer_to_operation_error)?;

        Ok(match selection {
            Some(index) => MenuSelection::Selected(index),
            None => MenuSelection::Cancelled,
        })
    }

    fn select_package_to_edit(
        &self,
        packages: &[&AdditionalPackageDeclaration],
    ) -> changeset_operations::Result<MenuSelection<usize>> {
        let items: Vec<String> = packages
            .iter()
            .map(|p| format!("{} ({})", p.name(), p.path().display()))
            .collect();

        let selection = Select::new()
            .with_prompt("Select package to edit")
            .items(&items)
            .interact_opt()
            .map_err(dialoguer_to_operation_error)?;

        Ok(match selection {
            Some(index) => MenuSelection::Selected(index),
            None => MenuSelection::Cancelled,
        })
    }

    fn select_field_to_edit(
        &self,
    ) -> changeset_operations::Result<MenuSelection<AdditionalPackageField>> {
        let options = [
            "path",
            "influence patterns",
            "manifest file path",
            "manifest format",
            "manifest version path",
            "Done",
        ];

        let selection = Select::new()
            .with_prompt("Which field would you like to edit?")
            .items(options)
            .interact_opt()
            .map_err(dialoguer_to_operation_error)?;

        Ok(match selection {
            Some(0) => MenuSelection::Selected(AdditionalPackageField::Path),
            Some(1) => MenuSelection::Selected(AdditionalPackageField::Influence),
            Some(2) => MenuSelection::Selected(AdditionalPackageField::ManifestFilePath),
            Some(3) => MenuSelection::Selected(AdditionalPackageField::ManifestFormat),
            Some(4) => MenuSelection::Selected(AdditionalPackageField::ManifestVersionPath),
            _ => MenuSelection::Cancelled,
        })
    }

    fn confirm_removal(&self, name: &str) -> changeset_operations::Result<bool> {
        Confirm::new()
            .with_prompt(format!("Remove additional package '{name}'?"))
            .default(false)
            .interact()
            .map_err(dialoguer_to_operation_error)
    }
}

pub(crate) fn run(args: AdditionalPackagesArgs, start_path: &Path) -> Result<()> {
    match args.command {
        AdditionalPackageCommand::Add(args) => run_add(args, start_path),
        AdditionalPackageCommand::Remove(args) => run_remove(args, start_path),
        AdditionalPackageCommand::Edit(args) => run_edit(args, start_path),
        AdditionalPackageCommand::List => run_list(start_path),
    }
}

fn run_add(args: AddAdditionalPackageArgs, start_path: &Path) -> Result<()> {
    let events = if let (
        Some(name),
        Some(path),
        Some(manifest_file),
        Some(manifest_format),
        Some(version_path),
    ) = (
        args.name,
        args.path,
        args.manifest_file,
        args.manifest_format,
        args.version_path,
    ) {
        let input = AdditionalPackageAddInput {
            name,
            path,
            influence: args.influence,
            manifest_file_path: manifest_file,
            manifest_format,
            manifest_version_path: version_path,
        };
        let op = AdditionalPackageDirectAddOperation::new(
            FileSystemProjectProvider::new(),
            FileSystemManifestWriter::new(),
        );
        op.execute(start_path, input)?
    } else if is_interactive() {
        let op = AdditionalPackageInteractiveAddOperation::new(
            FileSystemProjectProvider::new(),
            FileSystemManifestWriter::new(),
            TerminalAdditionalPackageInteractionProvider,
        );
        op.execute(start_path)?
    } else {
        return Err(CliError::NotATty);
    };

    print_additional_package_events(&events);
    Ok(())
}

fn run_remove(args: RemoveAdditionalPackageArgs, start_path: &Path) -> Result<()> {
    let events = if let Some(name) = args.name {
        let op = AdditionalPackageDirectRemoveOperation::new(
            FileSystemProjectProvider::new(),
            FileSystemManifestWriter::new(),
        );
        op.execute(start_path, &name)?
    } else if is_interactive() {
        let op = AdditionalPackageInteractiveRemoveOperation::new(
            FileSystemProjectProvider::new(),
            FileSystemManifestWriter::new(),
            TerminalAdditionalPackageInteractionProvider,
        );
        op.execute(start_path)?
    } else {
        return Err(CliError::NotATty);
    };

    print_additional_package_events(&events);
    Ok(())
}

fn run_edit(args: EditAdditionalPackageArgs, start_path: &Path) -> Result<()> {
    let has_updates = args.path.is_some()
        || !args.influence.is_empty()
        || args.manifest_file.is_some()
        || args.manifest_format.is_some()
        || args.version_path.is_some();

    let events = if let Some(name) = args.name.filter(|_| has_updates) {
        let updates = AdditionalPackageUpdate {
            path: args.path,
            influence: if args.influence.is_empty() {
                None
            } else {
                Some(args.influence)
            },
            manifest_file_path: args.manifest_file,
            manifest_format: args.manifest_format,
            manifest_version_path: args.version_path,
        };
        let input = AdditionalPackageEditInput { name, updates };
        let op = AdditionalPackageDirectEditOperation::new(
            FileSystemProjectProvider::new(),
            FileSystemManifestWriter::new(),
        );
        op.execute(start_path, input)?
    } else if is_interactive() {
        let op = AdditionalPackageInteractiveEditOperation::new(
            FileSystemProjectProvider::new(),
            FileSystemManifestWriter::new(),
            TerminalAdditionalPackageInteractionProvider,
        );
        op.execute(start_path)?
    } else {
        return Err(CliError::NotATty);
    };

    print_additional_package_events(&events);
    Ok(())
}

fn run_list(start_path: &Path) -> Result<()> {
    let op = AdditionalPackageListOperation::new(FileSystemProjectProvider::new());
    let events = op.execute(start_path)?;
    print_additional_package_events(&events);
    Ok(())
}

fn print_additional_package_events(events: &[AdditionalPackageEvent]) {
    for event in events {
        match event {
            AdditionalPackageEvent::Added { name } => {
                println!("Added additional package '{name}'");
            }
            AdditionalPackageEvent::Removed { name } => {
                println!("Removed additional package '{name}'");
            }
            AdditionalPackageEvent::Updated { name, field } => {
                println!("Updated additional package '{name}' (fields: {field})");
            }
            AdditionalPackageEvent::Listed(summaries) => {
                println!();
                println!("Additional packages:");
                for s in summaries {
                    println!("  {} ({})", s.name, s.path.display());
                    println!(
                        "    manifest: {} [{}]",
                        s.manifest_file_path.display(),
                        s.manifest_format
                    );
                }
                println!();
            }
            AdditionalPackageEvent::NotFound { name } => {
                println!("Additional package '{name}' not found");
            }
            AdditionalPackageEvent::AlreadyExists { name } => {
                println!("A package named '{name}' already exists");
            }
            AdditionalPackageEvent::NoAdditionalPackages => {
                println!("No additional packages configured.");
            }
        }
    }
}

fn dialoguer_to_operation_error(e: dialoguer::Error) -> OperationError {
    match e {
        dialoguer::Error::IO(io_err) => OperationError::Io(io_err),
    }
}
