use std::path::{Path, PathBuf};
use std::sync::Mutex;

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
    AdditionalPackageListOperation, VersionTrackingDependencyAddInput,
    VersionTrackingDependencyAddOperation, VersionTrackingDependencyEvent,
    VersionTrackingDependencyListOperation, VersionTrackingDependencyRemoveInput,
    VersionTrackingDependencyRemoveOperation,
};
use changeset_operations::providers::{FileSystemManifestWriter, FileSystemProjectProvider};
use changeset_operations::traits::{
    AdditionalPackageField, AdditionalPackageInteractionProvider, MenuSelection, ProjectProvider,
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
    /// Manage version-tracking dependencies for a package
    Dependencies(DependenciesArgs),
}

#[derive(Args)]
pub(crate) struct DependenciesArgs {
    #[command(subcommand)]
    pub(crate) command: DependencySubcommand,
}

#[derive(Subcommand)]
pub(crate) enum DependencySubcommand {
    /// Add a version-tracking dependency to a package
    Add(AddDependencyArgs),
    /// Remove a version-tracking dependency from a package
    Remove(RemoveDependencyArgs),
    /// List version-tracking dependencies for a package
    List(ListDependencyArgs),
}

#[derive(Args)]
pub(crate) struct AddDependencyArgs {
    /// Name of the package that owns the dependency
    #[arg(long)]
    pub package: Option<String>,

    /// Name of the dependency to track
    #[arg(long)]
    pub dependency: Option<String>,

    /// Path to the manifest file where the dependency version is tracked
    #[arg(long = "manifest-file")]
    pub manifest_file: Option<PathBuf>,

    /// Format of the manifest file (auto-detected from file extension if omitted)
    #[arg(long = "manifest-format", value_enum)]
    pub manifest_format: Option<ManifestFormat>,

    /// Path to the version field inside the manifest
    #[arg(long = "version-field-path")]
    pub version_field_path: Option<String>,
}

#[derive(Args)]
pub(crate) struct RemoveDependencyArgs {
    /// Name of the package that owns the dependency
    #[arg(long)]
    pub package: Option<String>,

    /// Name of the dependency to remove
    #[arg(long)]
    pub dependency: Option<String>,
}

#[derive(Args)]
pub(crate) struct ListDependencyArgs {
    /// Name of the package to list dependencies for
    #[arg(long)]
    pub package: Option<String>,
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
    #[arg(long = "version-field-path")]
    pub version_field_path: Option<String>,
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

    /// New version field path
    #[arg(long = "version-field-path")]
    pub version_field_path: Option<String>,
}

struct PackageDependenciesSelectionMenuItem {
    name: String,
    label: String,
}

#[derive(Default)]
struct TerminalAdditionalPackageInteractionProvider {
    default_manifest_path: Mutex<Option<PathBuf>>,
    last_manifest_path: Mutex<Option<PathBuf>>,
}

impl TerminalAdditionalPackageInteractionProvider {
    fn select_package(
        &self,
        prompt: &str,
        items: &[PackageDependenciesSelectionMenuItem],
    ) -> changeset_operations::Result<MenuSelection<String>> {
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();

        let selection = Select::new()
            .with_prompt(prompt)
            .items(&labels)
            .interact_opt()
            .map_err(super::dialoguer_to_operation_error)?;

        Ok(match selection {
            Some(index) => MenuSelection::Selected(items[index].name.clone()),
            None => MenuSelection::Cancelled,
        })
    }
}

impl AdditionalPackageInteractionProvider for TerminalAdditionalPackageInteractionProvider {
    fn prompt_package_name(&self) -> changeset_operations::Result<String> {
        Input::new()
            .with_prompt("Package name")
            .interact_text()
            .map_err(super::dialoguer_to_operation_error)
    }

    fn prompt_package_path(&self) -> changeset_operations::Result<PathBuf> {
        let s: String = Input::new()
            .with_prompt("Package directory path (relative to workspace root)")
            .interact_text()
            .map_err(super::dialoguer_to_operation_error)?;
        Ok(PathBuf::from(s))
    }

    fn prompt_influence_patterns(
        &self,
        package_path: &Path,
    ) -> changeset_operations::Result<Vec<String>> {
        Ok(crate::interaction::prompt_multi_value(
            &crate::interaction::MultiValuePromptConfig {
                intro: "Enter glob patterns for files that influence this package \
                        (one per line, empty line to finish):",
                first_prompt: "Glob pattern",
                additional_prompt: "Additional pattern",
                first_default: Some(format!("{}/**", package_path.display())),
            },
        )?)
    }

    fn prompt_manifest_file_path(&self) -> changeset_operations::Result<PathBuf> {
        let default_path = if let Ok(guard) = self.default_manifest_path.lock() {
            guard.as_ref().map(|p| p.display().to_string())
        } else {
            None
        };

        let input = Input::<String>::new().with_prompt("Path to version manifest file");
        let s: String = if let Some(default) = default_path {
            input.default(default)
        } else {
            input
        }
        .interact_text()
        .map_err(super::dialoguer_to_operation_error)?;

        let path = PathBuf::from(s);
        if let Ok(mut guard) = self.last_manifest_path.lock() {
            *guard = Some(path.clone());
        }
        Ok(path)
    }

    fn prompt_manifest_format(&self) -> changeset_operations::Result<ManifestFormat> {
        let variants = ManifestFormat::value_variants();
        let items: Vec<String> = variants.iter().map(ToString::to_string).collect();

        let default_index = if let Ok(guard) = self.last_manifest_path.lock() {
            guard
                .as_deref()
                .and_then(ManifestFormat::from_path)
                .and_then(|detected| variants.iter().position(|v| *v == detected))
                .unwrap_or(0)
        } else {
            0
        };

        let selection = Select::new()
            .with_prompt("Manifest format")
            .items(&items)
            .default(default_index)
            .interact_opt()
            .map_err(super::dialoguer_to_operation_error)?;

        match selection {
            Some(index) => Ok(variants[index]),
            None => Err(OperationError::Cancelled),
        }
    }

    fn prompt_manifest_version_field_path(&self) -> changeset_operations::Result<String> {
        Input::new()
            .with_prompt("Path to version field in manifest (e.g. 'version' or 'info.version')")
            .interact_text()
            .map_err(super::dialoguer_to_operation_error)
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
            .map_err(super::dialoguer_to_operation_error)?;

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
            .map_err(super::dialoguer_to_operation_error)?;

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
            .map_err(super::dialoguer_to_operation_error)?;

        Ok(match selection {
            Some(0) => MenuSelection::Selected(AdditionalPackageField::Path),
            Some(1) => MenuSelection::Selected(AdditionalPackageField::Influence),
            Some(2) => MenuSelection::Selected(AdditionalPackageField::ManifestFilePath),
            Some(3) => MenuSelection::Selected(AdditionalPackageField::ManifestFormat),
            Some(4) => MenuSelection::Selected(AdditionalPackageField::ManifestVersionFieldPath),
            _ => MenuSelection::Cancelled,
        })
    }

    fn confirm_removal(&self, name: &str) -> changeset_operations::Result<bool> {
        Confirm::new()
            .with_prompt(format!("Remove additional package '{name}'?"))
            .default(false)
            .interact()
            .map_err(super::dialoguer_to_operation_error)
    }
}

pub(crate) fn run(args: AdditionalPackagesArgs, start_path: &Path) -> Result<()> {
    match args.command {
        AdditionalPackageCommand::Add(args) => run_add(args, start_path),
        AdditionalPackageCommand::Remove(args) => run_remove(args, start_path),
        AdditionalPackageCommand::Edit(args) => run_edit(args, start_path),
        AdditionalPackageCommand::List => run_list(start_path),
        AdditionalPackageCommand::Dependencies(args) => run_dependencies(args, start_path),
    }
}

fn run_add(args: AddAdditionalPackageArgs, start_path: &Path) -> Result<()> {
    let manifest_format = args.manifest_format.or_else(|| {
        args.manifest_file
            .as_deref()
            .and_then(ManifestFormat::from_path)
    });

    let has_all_required = args.name.is_some()
        && args.path.is_some()
        && args.manifest_file.is_some()
        && args.version_field_path.is_some();

    let events = if let (
        Some(name),
        Some(path),
        Some(manifest_file),
        Some(format),
        Some(version_field_path),
    ) = (
        args.name,
        args.path,
        args.manifest_file,
        manifest_format,
        args.version_field_path,
    ) {
        let input = AdditionalPackageAddInput {
            name,
            path,
            influence: args.influence,
            manifest_file_path: manifest_file,
            manifest_format: format,
            manifest_version_field_path: version_field_path,
        };
        let op = AdditionalPackageDirectAddOperation::new(project_provider(), manifest_writer());
        op.execute(start_path, input)?
    } else if is_interactive() {
        let op = AdditionalPackageInteractiveAddOperation::new(
            project_provider(),
            manifest_writer(),
            TerminalAdditionalPackageInteractionProvider::default(),
        );
        op.execute(start_path)?
    } else if has_all_required {
        return Err(CliError::ManifestFormatRequired);
    } else {
        return Err(CliError::IncompleteArgs);
    };

    print_additional_package_events(&events);
    Ok(())
}

fn run_remove(args: RemoveAdditionalPackageArgs, start_path: &Path) -> Result<()> {
    let events = if let Some(name) = args.name {
        let op = AdditionalPackageDirectRemoveOperation::new(project_provider(), manifest_writer());
        op.execute(start_path, &name)?
    } else if is_interactive() {
        let op = AdditionalPackageInteractiveRemoveOperation::new(
            project_provider(),
            manifest_writer(),
            TerminalAdditionalPackageInteractionProvider::default(),
        );
        op.execute(start_path)?
    } else {
        return Err(CliError::IncompleteArgs);
    };

    print_additional_package_events(&events);
    Ok(())
}

fn run_edit(args: EditAdditionalPackageArgs, start_path: &Path) -> Result<()> {
    let has_updates = args.path.is_some()
        || !args.influence.is_empty()
        || args.manifest_file.is_some()
        || args.manifest_format.is_some()
        || args.version_field_path.is_some();

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
            manifest_version_field_path: args.version_field_path,
        };
        let input = AdditionalPackageEditInput { name, updates };
        let op = AdditionalPackageDirectEditOperation::new(project_provider(), manifest_writer());
        op.execute(start_path, input)?
    } else if is_interactive() {
        let op = AdditionalPackageInteractiveEditOperation::new(
            project_provider(),
            manifest_writer(),
            TerminalAdditionalPackageInteractionProvider::default(),
        );
        op.execute(start_path)?
    } else {
        return Err(CliError::IncompleteArgs);
    };

    print_additional_package_events(&events);
    Ok(())
}

fn run_list(start_path: &Path) -> Result<()> {
    let op = AdditionalPackageListOperation::new(project_provider());
    let events = op.execute(start_path)?;
    print_additional_package_events(&events);
    Ok(())
}

fn run_dependencies(args: DependenciesArgs, start_path: &Path) -> Result<()> {
    match args.command {
        DependencySubcommand::Add(args) => run_dependency_add(args, start_path),
        DependencySubcommand::Remove(args) => run_dependency_remove(args, start_path),
        DependencySubcommand::List(args) => run_dependency_list(args, start_path),
    }
}

fn run_dependency_add(args: AddDependencyArgs, start_path: &Path) -> Result<()> {
    let manifest_format = args.manifest_format.or_else(|| {
        args.manifest_file
            .as_deref()
            .and_then(ManifestFormat::from_path)
    });

    let has_all_required = args.package.is_some()
        && args.dependency.is_some()
        && args.manifest_file.is_some()
        && args.version_field_path.is_some();

    let events = if let (
        Some(package),
        Some(dependency),
        Some(manifest_file),
        Some(format),
        Some(version_field_path),
    ) = (
        args.package,
        args.dependency,
        args.manifest_file,
        manifest_format,
        args.version_field_path,
    ) {
        execute_dependency_add(
            package,
            dependency,
            manifest_file,
            format,
            version_field_path,
            start_path,
        )?
    } else if is_interactive() {
        let provider = project_provider();
        let project = provider.discover_project(start_path)?;
        let (root_config, _) = provider.load_configs(&project)?;
        let items = build_package_menu_items(project.packages(), root_config.additional_packages());

        let interaction = TerminalAdditionalPackageInteractionProvider::default();

        let package = match interaction.select_package("Select the owner package", &items)? {
            MenuSelection::Selected(name) => name,
            MenuSelection::Cancelled => return Ok(()),
        };

        if let Some(additional_pkg) = root_config
            .additional_packages()
            .iter()
            .find(|p| p.name() == &package)
            && let Ok(mut guard) = interaction.default_manifest_path.lock()
        {
            *guard = Some(additional_pkg.manifest().file_path().to_path_buf());
        }

        let dep_items: Vec<PackageDependenciesSelectionMenuItem> = items
            .into_iter()
            .filter(|item| item.name != package)
            .collect();

        if dep_items.is_empty() {
            println!("No other packages available to select as a dependency.");
            return Ok(());
        }

        let dependency =
            match interaction.select_package("Select the dependency to track", &dep_items)? {
                MenuSelection::Selected(name) => name,
                MenuSelection::Cancelled => return Ok(()),
            };

        let manifest_file = interaction.prompt_manifest_file_path()?;
        let format = interaction.prompt_manifest_format()?;
        let version_field_path = interaction.prompt_manifest_version_field_path()?;

        execute_dependency_add(
            package,
            dependency,
            manifest_file,
            format,
            version_field_path,
            start_path,
        )?
    } else if has_all_required {
        return Err(CliError::ManifestFormatRequired);
    } else {
        return Err(CliError::IncompleteArgs);
    };

    print_dependency_events(&events);
    Ok(())
}

fn run_dependency_remove(args: RemoveDependencyArgs, start_path: &Path) -> Result<()> {
    let events = if let (Some(package), Some(dependency)) = (args.package, args.dependency) {
        execute_dependency_remove(package, dependency, start_path)?
    } else if is_interactive() {
        let provider = project_provider();
        let project = provider.discover_project(start_path)?;
        let (root_config, pkg_configs) = provider.load_configs(&project)?;
        let items = build_package_menu_items(project.packages(), root_config.additional_packages());

        let interaction = TerminalAdditionalPackageInteractionProvider::default();

        let package = match interaction.select_package("Select package", &items)? {
            MenuSelection::Selected(name) => name,
            MenuSelection::Cancelled => return Ok(()),
        };

        let crate_deps = pkg_configs
            .get(&package)
            .map(|c| c.additional_package_dependencies())
            .unwrap_or_default();
        let dep_items = build_registered_dependency_items(
            &package,
            root_config.additional_packages(),
            crate_deps,
        );

        if dep_items.is_empty() {
            println!("No version-tracking dependencies configured for '{package}'.");
            return Ok(());
        }

        let dependency =
            match interaction.select_package("Select dependency to remove", &dep_items)? {
                MenuSelection::Selected(name) => name,
                MenuSelection::Cancelled => return Ok(()),
            };

        execute_dependency_remove(package, dependency, start_path)?
    } else {
        return Err(CliError::IncompleteArgs);
    };

    print_dependency_events(&events);
    Ok(())
}

fn run_dependency_list(args: ListDependencyArgs, start_path: &Path) -> Result<()> {
    let events = if let Some(package) = args.package {
        let op = VersionTrackingDependencyListOperation::new(project_provider());
        op.execute(start_path, &package)?
    } else if is_interactive() {
        let provider = project_provider();
        let project = provider.discover_project(start_path)?;
        let (root_config, _) = provider.load_configs(&project)?;
        let items = build_package_menu_items(project.packages(), root_config.additional_packages());

        let interaction = TerminalAdditionalPackageInteractionProvider::default();

        let package = match interaction.select_package("Select package", &items)? {
            MenuSelection::Selected(name) => name,
            MenuSelection::Cancelled => return Ok(()),
        };

        let op = VersionTrackingDependencyListOperation::new(provider);
        op.execute(start_path, &package)?
    } else {
        return Err(CliError::IncompleteArgs);
    };

    print_dependency_events(&events);
    Ok(())
}

fn execute_dependency_add(
    package: String,
    dependency: String,
    manifest_file: PathBuf,
    manifest_format: ManifestFormat,
    version_field_path: String,
    start_path: &Path,
) -> Result<Vec<VersionTrackingDependencyEvent>> {
    let input = VersionTrackingDependencyAddInput::new(
        package,
        dependency,
        manifest_file,
        manifest_format,
        version_field_path,
    );
    let op = VersionTrackingDependencyAddOperation::new(project_provider(), manifest_writer());
    Ok(op.execute(start_path, input)?)
}

fn execute_dependency_remove(
    package: String,
    dependency: String,
    start_path: &Path,
) -> Result<Vec<VersionTrackingDependencyEvent>> {
    let input = VersionTrackingDependencyRemoveInput::new(package, dependency);
    let op = VersionTrackingDependencyRemoveOperation::new(project_provider(), manifest_writer());
    Ok(op.execute(start_path, input)?)
}

fn print_dependency_events(events: &[VersionTrackingDependencyEvent]) {
    for event in events {
        match event {
            VersionTrackingDependencyEvent::Added {
                package_name,
                dependency_name,
            } => {
                println!(
                    "Added version-tracking dependency '{dependency_name}' to package '{package_name}'"
                );
            }
            VersionTrackingDependencyEvent::Removed {
                package_name,
                dependency_name,
            } => {
                println!(
                    "Removed version-tracking dependency '{dependency_name}' from package '{package_name}'"
                );
            }
            VersionTrackingDependencyEvent::Listed(summaries) => {
                println!();
                println!("Version-tracking dependencies:");
                for s in summaries {
                    println!(
                        "  {} -> {} [{}]",
                        s.dependency_name(),
                        s.manifest_file_path().display(),
                        s.manifest_format()
                    );
                    println!("    version field: {}", s.version_field_path());
                }
                println!();
            }
            VersionTrackingDependencyEvent::NoDependencies { package_name } => {
                println!("No version-tracking dependencies configured for '{package_name}'.");
            }
        }
    }
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

fn build_package_menu_items(
    crate_packages: &[changeset_core::PackageInfo],
    additional_packages: &[AdditionalPackageDeclaration],
) -> Vec<PackageDependenciesSelectionMenuItem> {
    let mut items: Vec<PackageDependenciesSelectionMenuItem> = crate_packages
        .iter()
        .map(|p| PackageDependenciesSelectionMenuItem {
            name: p.name().clone(),
            label: format!("{} (crate)", p.name()),
        })
        .collect();

    for pkg in additional_packages {
        items.push(PackageDependenciesSelectionMenuItem {
            name: pkg.name().clone(),
            label: format!("{} (additional: {})", pkg.name(), pkg.path().display()),
        });
    }

    items
}

fn build_registered_dependency_items(
    package_name: &str,
    additional_packages: &[AdditionalPackageDeclaration],
    crate_deps: &[changeset_core::VersionTrackingDependency],
) -> Vec<PackageDependenciesSelectionMenuItem> {
    if let Some(additional) = additional_packages
        .iter()
        .find(|p| p.name() == package_name)
    {
        return additional
            .dependencies()
            .iter()
            .map(|dep| PackageDependenciesSelectionMenuItem {
                name: dep.dependency_name().clone(),
                label: dep.dependency_name().clone(),
            })
            .collect();
    }

    crate_deps
        .iter()
        .map(|dep| PackageDependenciesSelectionMenuItem {
            name: dep.dependency_name().clone(),
            label: dep.dependency_name().clone(),
        })
        .collect()
}

fn project_provider() -> FileSystemProjectProvider {
    FileSystemProjectProvider::new()
}

fn manifest_writer() -> FileSystemManifestWriter {
    FileSystemManifestWriter::new()
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use changeset_core::{
        AdditionalPackageDeclaration, AdditionalPackageManifest, ManifestFormat, PackageInfo,
        VersionTrackingDependency, VersionTrackingManifest,
    };
    use semver::Version;

    use super::{build_package_menu_items, build_registered_dependency_items};

    fn sample_crate_packages() -> Vec<PackageInfo> {
        vec![
            PackageInfo::new(
                "crate-a".to_string(),
                Version::new(1, 0, 0),
                PathBuf::from("crates/crate-a"),
            ),
            PackageInfo::new(
                "crate-b".to_string(),
                Version::new(0, 2, 0),
                PathBuf::from("crates/crate-b"),
            ),
        ]
    }

    fn sample_additional_package(
        name: &str,
        path: &str,
        deps: Vec<VersionTrackingDependency>,
    ) -> AdditionalPackageDeclaration {
        AdditionalPackageDeclaration::new(
            name.to_string(),
            PathBuf::from(path),
            vec![format!("{path}/**")],
            AdditionalPackageManifest::new(
                PathBuf::from(format!("{path}/Chart.yaml")),
                ManifestFormat::Yaml,
                "version".to_string(),
            ),
            deps,
        )
    }

    fn sample_version_tracking_dependency(name: &str) -> VersionTrackingDependency {
        VersionTrackingDependency::new(
            name.to_string(),
            VersionTrackingManifest::new(
                PathBuf::from("manifest.yaml"),
                ManifestFormat::Yaml,
                "dependencies.version".to_string(),
            ),
        )
    }

    #[test]
    fn build_menu_items_includes_crates() {
        let crates = sample_crate_packages();

        let items = build_package_menu_items(&crates, &[]);

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].name, "crate-a");
        assert!(items[0].label.contains("(crate)"));
        assert_eq!(items[1].name, "crate-b");
        assert!(items[1].label.contains("(crate)"));
    }

    #[test]
    fn build_menu_items_includes_additional_packages() {
        let crates = sample_crate_packages();
        let additional = vec![sample_additional_package(
            "my-chart",
            "charts/my-chart",
            vec![],
        )];

        let items = build_package_menu_items(&crates, &additional);

        assert_eq!(items.len(), 3);
        assert_eq!(items[2].name, "my-chart");
        assert!(items[2].label.contains("(additional:"));
        assert!(items[2].label.contains("charts/my-chart"));
    }

    #[test]
    fn build_menu_items_empty_when_no_packages() {
        let items = build_package_menu_items(&[], &[]);

        assert!(items.is_empty());
    }

    #[test]
    fn registered_deps_from_additional_package() {
        let dep = sample_version_tracking_dependency("crate-a");
        let additional = vec![sample_additional_package(
            "my-chart",
            "charts/my-chart",
            vec![dep],
        )];

        let items = build_registered_dependency_items("my-chart", &additional, &[]);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "crate-a");
    }

    #[test]
    fn registered_deps_from_crate_deps() {
        let dep = sample_version_tracking_dependency("my-chart");

        let items = build_registered_dependency_items("crate-a", &[], &[dep]);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "my-chart");
    }

    #[test]
    fn registered_deps_returns_empty_when_no_sources() {
        let items = build_registered_dependency_items("nonexistent", &[], &[]);

        assert!(items.is_empty());
    }

    #[test]
    fn registered_deps_returns_empty_when_additional_package_has_no_deps() {
        let additional = vec![sample_additional_package(
            "my-chart",
            "charts/my-chart",
            vec![],
        )];

        let items = build_registered_dependency_items("my-chart", &additional, &[]);

        assert!(items.is_empty());
    }

    #[test]
    fn registered_deps_prefers_additional_package_over_crate_deps() {
        let dep_from_additional = sample_version_tracking_dependency("from-additional");
        let dep_from_crate = sample_version_tracking_dependency("from-crate");
        let additional = vec![sample_additional_package(
            "dual-pkg",
            "charts/dual-pkg",
            vec![dep_from_additional],
        )];

        let items = build_registered_dependency_items("dual-pkg", &additional, &[dep_from_crate]);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "from-additional");
    }

    #[test]
    fn manifest_format_from_path_detects_toml() {
        assert_eq!(
            ManifestFormat::from_path(Path::new("Cargo.toml")),
            Some(ManifestFormat::Toml)
        );
    }

    #[test]
    fn manifest_format_from_path_detects_yaml() {
        assert_eq!(
            ManifestFormat::from_path(Path::new("Chart.yaml")),
            Some(ManifestFormat::Yaml)
        );
    }

    #[test]
    fn manifest_format_from_path_detects_yml() {
        assert_eq!(
            ManifestFormat::from_path(Path::new("config.yml")),
            Some(ManifestFormat::Yaml)
        );
    }

    #[test]
    fn manifest_format_from_path_detects_json() {
        assert_eq!(
            ManifestFormat::from_path(Path::new("package.json")),
            Some(ManifestFormat::Json)
        );
    }

    #[test]
    fn manifest_format_from_path_returns_none_for_unknown_extension() {
        assert_eq!(ManifestFormat::from_path(Path::new("readme.txt")), None);
    }

    #[test]
    fn manifest_format_from_path_returns_none_for_no_extension() {
        assert_eq!(ManifestFormat::from_path(Path::new("Makefile")), None);
    }
}
