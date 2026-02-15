use std::io::IsTerminal;
use std::path::Path;

use changeset_core::PrereleaseSpec;
use changeset_operations::providers::{FileSystemProjectProvider, FileSystemReleaseStateIO};
use changeset_operations::traits::{ProjectProvider, ReleaseStateIO};
use changeset_project::{CargoProject, GraduationState, PrereleaseState};
use changeset_version::{is_prerelease, is_zero_version};
use dialoguer::{Input, Select};

use super::{ManageArgs, ManageCommand, ManageGraduationArgs, ManagePrereleaseArgs};
use crate::error::{CliError, Result};

pub(crate) fn run(args: ManageArgs, start_path: &Path) -> Result<()> {
    match args.command {
        ManageCommand::Prerelease(prerelease_args) => run_prerelease(prerelease_args, start_path),
        ManageCommand::Graduation(graduation_args) => run_graduation(graduation_args, start_path),
    }
}

fn run_prerelease(args: ManagePrereleaseArgs, start_path: &Path) -> Result<()> {
    let project_provider = FileSystemProjectProvider::new();
    let project = project_provider.discover_project(start_path)?;
    let (root_config, _) = project_provider.load_configs(&project)?;
    let changeset_dir = project.root.join(root_config.changeset_dir());

    let release_state_io = FileSystemReleaseStateIO::new();
    let mut prerelease_state = release_state_io
        .load_prerelease_state(&changeset_dir)?
        .unwrap_or_default();

    let mut graduation_state = release_state_io
        .load_graduation_state(&changeset_dir)?
        .unwrap_or_default();

    let no_flags_provided =
        args.add.is_empty() && args.remove.is_empty() && args.graduate.is_empty() && !args.list;

    if no_flags_provided {
        run_prerelease_interactive(
            &project,
            &changeset_dir,
            &release_state_io,
            &mut prerelease_state,
            &mut graduation_state,
        )?;
        return Ok(());
    }

    let mut modified_prerelease = false;
    let mut modified_graduation = false;

    for entry in &args.add {
        let (crate_name, tag) = parse_prerelease_entry(entry)?;
        validate_package_exists(&project, &crate_name)?;
        validate_prerelease_tag(&tag)?;

        prerelease_state.insert(crate_name.clone(), tag);
        modified_prerelease = true;
        println!("Added {crate_name} to pre-release configuration");
    }

    for crate_name in &args.remove {
        if prerelease_state.remove(crate_name).is_some() {
            modified_prerelease = true;
            println!("Removed {crate_name} from pre-release configuration");
        }
    }

    for crate_name in &args.graduate {
        validate_package_exists(&project, crate_name)?;
        validate_can_graduate(&project, crate_name)?;

        if prerelease_state.remove(crate_name).is_some() {
            modified_prerelease = true;
        }

        graduation_state.add(crate_name.clone());
        modified_graduation = true;
        println!("Moved {crate_name} to graduation queue");
    }

    if modified_prerelease {
        release_state_io.save_prerelease_state(&changeset_dir, &prerelease_state)?;
    }
    if modified_graduation {
        release_state_io.save_graduation_state(&changeset_dir, &graduation_state)?;
    }

    if args.list {
        print_prerelease_state(&prerelease_state);
    }

    Ok(())
}

fn run_prerelease_interactive(
    project: &CargoProject,
    changeset_dir: &Path,
    release_state_io: &FileSystemReleaseStateIO,
    prerelease_state: &mut PrereleaseState,
    graduation_state: &mut GraduationState,
) -> Result<()> {
    if !is_interactive() {
        return Err(CliError::NotATty);
    }

    loop {
        println!();
        print_prerelease_state(prerelease_state);
        println!();

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
            .map_err(dialoguer_to_cli_error)?;

        match selection {
            Some(0) => {
                interactive_add_prerelease(
                    project,
                    changeset_dir,
                    release_state_io,
                    prerelease_state,
                )?;
            }
            Some(1) => {
                interactive_remove_prerelease(changeset_dir, release_state_io, prerelease_state)?;
            }
            Some(2) => {
                interactive_graduate_from_prerelease(
                    project,
                    changeset_dir,
                    release_state_io,
                    prerelease_state,
                    graduation_state,
                )?;
            }
            Some(3) | None => break,
            _ => {}
        }
    }

    Ok(())
}

fn interactive_add_prerelease(
    project: &CargoProject,
    changeset_dir: &Path,
    release_state_io: &FileSystemReleaseStateIO,
    prerelease_state: &mut PrereleaseState,
) -> Result<()> {
    let available: Vec<_> = project
        .packages
        .iter()
        .filter(|p| !prerelease_state.contains(&p.name))
        .collect();

    if available.is_empty() {
        println!("All packages are already in pre-release mode.");
        return Ok(());
    }

    let items: Vec<String> = available
        .iter()
        .map(|p| format!("{} ({})", p.name, p.version))
        .collect();

    let selection = Select::new()
        .with_prompt("Select a crate to add to pre-release")
        .items(&items)
        .interact_opt()
        .map_err(dialoguer_to_cli_error)?;

    let Some(index) = selection else {
        return Ok(());
    };

    let crate_name = &available[index].name;

    let tag: String = Input::new()
        .with_prompt("Enter pre-release tag (e.g., alpha, beta, rc)")
        .interact_text()
        .map_err(dialoguer_to_cli_error)?;

    validate_prerelease_tag(&tag)?;

    prerelease_state.insert(crate_name.clone(), tag.clone());
    release_state_io.save_prerelease_state(changeset_dir, prerelease_state)?;
    println!("Added {crate_name} to pre-release configuration with tag '{tag}'");

    Ok(())
}

fn interactive_remove_prerelease(
    changeset_dir: &Path,
    release_state_io: &FileSystemReleaseStateIO,
    prerelease_state: &mut PrereleaseState,
) -> Result<()> {
    if prerelease_state.is_empty() {
        println!("No packages are currently in pre-release mode.");
        return Ok(());
    }

    let mut items: Vec<_> = prerelease_state
        .iter()
        .map(|(name, tag)| (name.to_string(), format!("{name}: {tag}")))
        .collect();
    items.sort_by(|a, b| a.0.cmp(&b.0));

    let display_items: Vec<&str> = items.iter().map(|(_, display)| display.as_str()).collect();

    let selection = Select::new()
        .with_prompt("Select a crate to remove from pre-release")
        .items(&display_items)
        .interact_opt()
        .map_err(dialoguer_to_cli_error)?;

    let Some(index) = selection else {
        return Ok(());
    };

    let crate_name = items[index].0.clone();
    let _ = prerelease_state.remove(&crate_name);
    release_state_io.save_prerelease_state(changeset_dir, prerelease_state)?;
    println!("Removed {crate_name} from pre-release configuration");

    Ok(())
}

fn interactive_graduate_from_prerelease(
    project: &CargoProject,
    changeset_dir: &Path,
    release_state_io: &FileSystemReleaseStateIO,
    prerelease_state: &mut PrereleaseState,
    graduation_state: &mut GraduationState,
) -> Result<()> {
    let eligible: Vec<_> = project
        .packages
        .iter()
        .filter(|p| is_zero_version(&p.version) && !is_prerelease(&p.version))
        .collect();

    if eligible.is_empty() {
        println!("No eligible packages for graduation (must be 0.x stable version).");
        return Ok(());
    }

    let items: Vec<String> = eligible
        .iter()
        .map(|p| format!("{} ({})", p.name, p.version))
        .collect();

    let selection = Select::new()
        .with_prompt("Select a crate to graduate (move to graduation queue)")
        .items(&items)
        .interact_opt()
        .map_err(dialoguer_to_cli_error)?;

    let Some(index) = selection else {
        return Ok(());
    };

    let crate_name = &eligible[index].name;

    if prerelease_state.remove(crate_name).is_some() {
        release_state_io.save_prerelease_state(changeset_dir, prerelease_state)?;
    }

    graduation_state.add(crate_name.clone());
    release_state_io.save_graduation_state(changeset_dir, graduation_state)?;
    println!("Moved {crate_name} to graduation queue");

    Ok(())
}

fn run_graduation(args: ManageGraduationArgs, start_path: &Path) -> Result<()> {
    let project_provider = FileSystemProjectProvider::new();
    let project = project_provider.discover_project(start_path)?;
    let (root_config, _) = project_provider.load_configs(&project)?;
    let changeset_dir = project.root.join(root_config.changeset_dir());

    let release_state_io = FileSystemReleaseStateIO::new();
    let mut state = release_state_io
        .load_graduation_state(&changeset_dir)?
        .unwrap_or_default();

    let no_flags_provided = args.add.is_empty() && args.remove.is_empty() && !args.list;

    if no_flags_provided {
        run_graduation_interactive(&project, &changeset_dir, &release_state_io, &mut state)?;
        return Ok(());
    }

    let mut modified = false;

    for crate_name in &args.add {
        validate_package_exists(&project, crate_name)?;
        validate_can_graduate(&project, crate_name)?;

        state.add(crate_name.clone());
        modified = true;
        println!("Added {crate_name} to graduation queue");
    }

    for crate_name in &args.remove {
        if state.remove(crate_name) {
            modified = true;
            println!("Removed {crate_name} from graduation queue");
        }
    }

    if modified {
        release_state_io.save_graduation_state(&changeset_dir, &state)?;
    }

    if args.list {
        print_graduation_state(&state);
    }

    Ok(())
}

fn run_graduation_interactive(
    project: &CargoProject,
    changeset_dir: &Path,
    release_state_io: &FileSystemReleaseStateIO,
    state: &mut GraduationState,
) -> Result<()> {
    if !is_interactive() {
        return Err(CliError::NotATty);
    }

    loop {
        println!();
        print_graduation_state(state);
        println!();

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
            .map_err(dialoguer_to_cli_error)?;

        match selection {
            Some(0) => {
                interactive_add_graduation(project, changeset_dir, release_state_io, state)?;
            }
            Some(1) => {
                interactive_remove_graduation(changeset_dir, release_state_io, state)?;
            }
            Some(2) | None => break,
            _ => {}
        }
    }

    Ok(())
}

fn interactive_add_graduation(
    project: &CargoProject,
    changeset_dir: &Path,
    release_state_io: &FileSystemReleaseStateIO,
    state: &mut GraduationState,
) -> Result<()> {
    let eligible: Vec<_> = project
        .packages
        .iter()
        .filter(|p| {
            is_zero_version(&p.version) && !is_prerelease(&p.version) && !state.contains(&p.name)
        })
        .collect();

    if eligible.is_empty() {
        println!(
            "No eligible packages for graduation (must be 0.x stable version and not already queued)."
        );
        return Ok(());
    }

    let items: Vec<String> = eligible
        .iter()
        .map(|p| format!("{} ({})", p.name, p.version))
        .collect();

    let selection = Select::new()
        .with_prompt("Select a crate to add to graduation queue")
        .items(&items)
        .interact_opt()
        .map_err(dialoguer_to_cli_error)?;

    let Some(index) = selection else {
        return Ok(());
    };

    let crate_name = &eligible[index].name;
    state.add(crate_name.clone());
    release_state_io.save_graduation_state(changeset_dir, state)?;
    println!("Added {crate_name} to graduation queue");

    Ok(())
}

fn interactive_remove_graduation(
    changeset_dir: &Path,
    release_state_io: &FileSystemReleaseStateIO,
    state: &mut GraduationState,
) -> Result<()> {
    if state.is_empty() {
        println!("No packages are currently queued for graduation.");
        return Ok(());
    }

    let mut items: Vec<String> = state.iter().map(str::to_string).collect();
    items.sort();

    let selection = Select::new()
        .with_prompt("Select a crate to remove from graduation queue")
        .items(&items)
        .interact_opt()
        .map_err(dialoguer_to_cli_error)?;

    let Some(index) = selection else {
        return Ok(());
    };

    let crate_name = &items[index];
    let _ = state.remove(crate_name);
    release_state_io.save_graduation_state(changeset_dir, state)?;
    println!("Removed {crate_name} from graduation queue");

    Ok(())
}

/// Returns true if the terminal supports interactive prompts.
///
/// Checks two conditions:
/// - `CARGO_CHANGESET_FORCE_TTY` environment variable is set (for testing)
/// - Standard input is a terminal (for normal usage)
fn is_interactive() -> bool {
    std::env::var("CARGO_CHANGESET_FORCE_TTY").is_ok() || std::io::stdin().is_terminal()
}

fn dialoguer_to_cli_error(e: dialoguer::Error) -> CliError {
    match e {
        dialoguer::Error::IO(io_err) => CliError::Io(io_err),
    }
}

fn parse_prerelease_entry(input: &str) -> Result<(String, String)> {
    let Some((crate_name, tag)) = input.split_once(':') else {
        return Err(CliError::InvalidPrereleaseFormat {
            input: input.to_string(),
        });
    };

    if crate_name.is_empty() || tag.is_empty() {
        return Err(CliError::InvalidPrereleaseFormat {
            input: input.to_string(),
        });
    }

    Ok((crate_name.to_string(), tag.to_string()))
}

fn validate_prerelease_tag(tag: &str) -> Result<()> {
    tag.parse::<PrereleaseSpec>()
        .map_err(|_| CliError::InvalidPrereleaseTag {
            tag: tag.to_string(),
        })?;
    Ok(())
}

fn validate_package_exists(project: &CargoProject, name: &str) -> Result<()> {
    if !project.packages.iter().any(|p| p.name == name) {
        return Err(CliError::PackageNotFound {
            name: name.to_string(),
        });
    }
    Ok(())
}

fn validate_can_graduate(project: &CargoProject, name: &str) -> Result<()> {
    let package = project
        .packages
        .iter()
        .find(|p| p.name == name)
        .ok_or_else(|| CliError::PackageNotFound {
            name: name.to_string(),
        })?;

    if is_prerelease(&package.version) {
        return Err(CliError::CannotGraduatePrerelease {
            package: name.to_string(),
            version: package.version.to_string(),
        });
    }

    if !is_zero_version(&package.version) {
        return Err(CliError::CannotGraduateStable {
            package: name.to_string(),
            version: package.version.to_string(),
        });
    }

    Ok(())
}

fn print_prerelease_state(state: &PrereleaseState) {
    if state.is_empty() {
        println!("(No packages in pre-release mode)");
        return;
    }

    println!("Pre-release configuration (.changeset/pre-release.toml):");
    let mut items: Vec<_> = state.iter().collect();
    items.sort_by(|a, b| a.0.cmp(b.0));
    for (crate_name, tag) in items {
        println!("  {crate_name}: {tag}");
    }
}

fn print_graduation_state(state: &GraduationState) {
    if state.is_empty() {
        println!("(No packages queued for graduation)");
        return;
    }

    println!("Graduation queue (.changeset/graduation.toml):");
    let mut items: Vec<_> = state.iter().collect();
    items.sort();
    for crate_name in items {
        println!("  - {crate_name}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod parse_prerelease_entry {
        use super::*;

        #[test]
        fn parses_valid_format() {
            let result = parse_prerelease_entry("my-crate:alpha");

            assert!(result.is_ok());
            let (name, tag) = result.expect("should parse");
            assert_eq!(name, "my-crate");
            assert_eq!(tag, "alpha");
        }

        #[test]
        fn parses_custom_tag() {
            let result = parse_prerelease_entry("crate-name:nightly");

            assert!(result.is_ok());
            let (name, tag) = result.expect("should parse");
            assert_eq!(name, "crate-name");
            assert_eq!(tag, "nightly");
        }

        #[test]
        fn rejects_missing_colon() {
            let result = parse_prerelease_entry("no-colon-here");

            assert!(result.is_err());
            assert!(matches!(
                result.unwrap_err(),
                CliError::InvalidPrereleaseFormat { .. }
            ));
        }

        #[test]
        fn rejects_empty_crate_name() {
            let result = parse_prerelease_entry(":alpha");

            assert!(result.is_err());
            assert!(matches!(
                result.unwrap_err(),
                CliError::InvalidPrereleaseFormat { .. }
            ));
        }

        #[test]
        fn rejects_empty_tag() {
            let result = parse_prerelease_entry("crate-name:");

            assert!(result.is_err());
            assert!(matches!(
                result.unwrap_err(),
                CliError::InvalidPrereleaseFormat { .. }
            ));
        }

        #[test]
        fn handles_multiple_colons() {
            let result = parse_prerelease_entry("crate:tag:extra");

            assert!(result.is_ok());
            let (name, tag) = result.expect("should parse");
            assert_eq!(name, "crate");
            assert_eq!(tag, "tag:extra");
        }
    }

    mod validate_prerelease_tag {
        use super::*;

        #[test]
        fn accepts_alpha() {
            assert!(validate_prerelease_tag("alpha").is_ok());
        }

        #[test]
        fn accepts_beta() {
            assert!(validate_prerelease_tag("beta").is_ok());
        }

        #[test]
        fn accepts_rc() {
            assert!(validate_prerelease_tag("rc").is_ok());
        }

        #[test]
        fn accepts_custom_alphanumeric() {
            assert!(validate_prerelease_tag("nightly").is_ok());
            assert!(validate_prerelease_tag("dev123").is_ok());
        }

        #[test]
        fn accepts_hyphenated_tags() {
            assert!(validate_prerelease_tag("pre-release").is_ok());
        }

        #[test]
        fn rejects_empty() {
            let result = validate_prerelease_tag("");

            assert!(result.is_err());
        }

        #[test]
        fn rejects_invalid_characters() {
            let result = validate_prerelease_tag("alpha.1");

            assert!(result.is_err());
        }

        #[test]
        fn rejects_spaces() {
            let result = validate_prerelease_tag("alpha 1");

            assert!(result.is_err());
            assert!(matches!(
                result.unwrap_err(),
                CliError::InvalidPrereleaseTag { .. }
            ));
        }

        #[test]
        fn rejects_underscores() {
            let result = validate_prerelease_tag("alpha_1");

            assert!(result.is_err());
        }
    }

    mod validate_package_exists {
        use super::*;
        use changeset_core::PackageInfo;
        use changeset_project::CargoProject;
        use std::path::PathBuf;

        fn make_project(packages: Vec<(&str, &str)>) -> CargoProject {
            CargoProject {
                root: PathBuf::from("/mock/project"),
                kind: changeset_project::ProjectKind::VirtualWorkspace,
                packages: packages
                    .into_iter()
                    .map(|(name, version)| PackageInfo {
                        name: name.to_string(),
                        version: version.parse().expect("valid version"),
                        path: PathBuf::from(format!("/mock/project/crates/{name}")),
                    })
                    .collect(),
            }
        }

        #[test]
        fn succeeds_for_existing_package() {
            let project = make_project(vec![("crate-a", "1.0.0"), ("crate-b", "2.0.0")]);

            let result = validate_package_exists(&project, "crate-a");

            assert!(result.is_ok());
        }

        #[test]
        fn fails_for_unknown_package() {
            let project = make_project(vec![("crate-a", "1.0.0")]);

            let result = validate_package_exists(&project, "nonexistent");

            assert!(result.is_err());
            let err = result.unwrap_err();
            assert!(matches!(err, CliError::PackageNotFound { .. }));
            assert!(err.to_string().contains("nonexistent"));
        }

        #[test]
        fn fails_for_empty_project() {
            let project = make_project(vec![]);

            let result = validate_package_exists(&project, "any-crate");

            assert!(result.is_err());
            assert!(matches!(
                result.unwrap_err(),
                CliError::PackageNotFound { .. }
            ));
        }
    }

    mod validate_can_graduate {
        use super::*;
        use changeset_core::PackageInfo;
        use changeset_project::CargoProject;
        use std::path::PathBuf;

        fn make_project(packages: Vec<(&str, &str)>) -> CargoProject {
            CargoProject {
                root: PathBuf::from("/mock/project"),
                kind: changeset_project::ProjectKind::VirtualWorkspace,
                packages: packages
                    .into_iter()
                    .map(|(name, version)| PackageInfo {
                        name: name.to_string(),
                        version: version.parse().expect("valid version"),
                        path: PathBuf::from(format!("/mock/project/crates/{name}")),
                    })
                    .collect(),
            }
        }

        #[test]
        fn succeeds_for_zero_stable_version() {
            let project = make_project(vec![("crate-a", "0.5.0")]);

            let result = validate_can_graduate(&project, "crate-a");

            assert!(result.is_ok());
        }

        #[test]
        fn fails_for_prerelease_version() {
            let project = make_project(vec![("crate-a", "0.5.0-alpha.1")]);

            let result = validate_can_graduate(&project, "crate-a");

            assert!(result.is_err());
            let err = result.unwrap_err();
            assert!(matches!(err, CliError::CannotGraduatePrerelease { .. }));
            assert!(err.to_string().contains("crate-a"));
            assert!(err.to_string().contains("prerelease"));
        }

        #[test]
        fn fails_for_stable_version_1_0_0() {
            let project = make_project(vec![("crate-a", "1.0.0")]);

            let result = validate_can_graduate(&project, "crate-a");

            assert!(result.is_err());
            let err = result.unwrap_err();
            assert!(matches!(err, CliError::CannotGraduateStable { .. }));
            assert!(err.to_string().contains("stable"));
        }

        #[test]
        fn fails_for_stable_version_above_1() {
            let project = make_project(vec![("crate-a", "2.5.3")]);

            let result = validate_can_graduate(&project, "crate-a");

            assert!(result.is_err());
            assert!(matches!(
                result.unwrap_err(),
                CliError::CannotGraduateStable { .. }
            ));
        }

        #[test]
        fn fails_for_unknown_package() {
            let project = make_project(vec![("crate-a", "0.5.0")]);

            let result = validate_can_graduate(&project, "nonexistent");

            assert!(result.is_err());
            assert!(matches!(
                result.unwrap_err(),
                CliError::PackageNotFound { .. }
            ));
        }

        #[test]
        fn fails_for_zero_prerelease_version() {
            let project = make_project(vec![("crate-a", "0.1.0-beta.1")]);

            let result = validate_can_graduate(&project, "crate-a");

            assert!(result.is_err());
            assert!(matches!(
                result.unwrap_err(),
                CliError::CannotGraduatePrerelease { .. }
            ));
        }
    }

    mod dialoguer_conversion {
        use super::*;

        #[test]
        fn converts_io_error() {
            let io_err = dialoguer::Error::IO(std::io::Error::other("test error"));

            let cli_err = dialoguer_to_cli_error(io_err);

            assert!(matches!(cli_err, CliError::Io(_)));
        }
    }
}
