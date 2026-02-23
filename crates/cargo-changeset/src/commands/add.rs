use std::collections::HashMap;
use std::io::Read as _;
use std::path::Path;

use crate::environment::is_interactive;

use changeset_core::BumpType;
use changeset_operations::operations::{AddInput, AddOperation, AddResult};
use changeset_operations::providers::{FileSystemChangesetIO, FileSystemProjectProvider};
use changeset_operations::traits::ProjectProvider;
use changeset_project::ProjectKind;

use super::AddArgs;
use crate::error::{CliError, Result};
use crate::interaction::{NonInteractiveProvider, TerminalInteractionProvider};

pub(super) fn run(args: AddArgs, start_path: &Path) -> Result<()> {
    validate_package_bump_args(&args.package_bumps)?;

    let project_provider = FileSystemProjectProvider::new();
    let project = project_provider.discover_project(start_path)?;

    let is_single_package = project.kind == ProjectKind::SinglePackage && args.packages.is_empty();
    if is_single_package {
        if let Some(pkg) = project.packages.first() {
            println!("Using package: {} ({})", pkg.name, pkg.version);
        }
    }

    let changeset_writer = FileSystemChangesetIO::new(&project.root);

    let input = build_input(&args)?;

    let result = if is_interactive() {
        let interaction_provider = TerminalInteractionProvider::new(args.editor);
        let operation = AddOperation::new(project_provider, changeset_writer, interaction_provider);
        operation.execute(start_path, input)?
    } else {
        let interaction_provider = NonInteractiveProvider;
        let operation = AddOperation::new(project_provider, changeset_writer, interaction_provider);
        operation.execute(start_path, input)?
    };

    match result {
        AddResult::Created {
            changeset,
            file_path,
        } => {
            println!();
            println!("Created changeset: {}", file_path.display());
            println!();
            println!("Summary: {}", changeset.summary);
            println!("Category: {}", changeset.category);
            println!();
            println!("Releases:");
            for release in &changeset.releases {
                println!("  - {}: {:?}", release.name, release.bump_type);
            }
            Ok(())
        }
        AddResult::Cancelled | AddResult::NoPackages => Ok(()),
    }
}

fn build_input(args: &AddArgs) -> Result<AddInput> {
    let package_bumps = parse_package_bumps(&args.package_bumps)?;

    let description = match &args.message {
        Some(message) if message == "-" => Some(read_description_from_stdin()?),
        Some(message) => Some(message.clone()),
        None => None,
    };

    Ok(AddInput {
        packages: args.packages.clone(),
        bump: args.bump,
        package_bumps,
        category: args.category,
        description,
    })
}

fn validate_package_bump_args(package_bumps: &[String]) -> Result<()> {
    for input in package_bumps {
        parse_package_bump(input)?;
    }
    Ok(())
}

fn parse_package_bumps(package_bumps: &[String]) -> Result<HashMap<String, BumpType>> {
    let mut map = HashMap::new();

    for input in package_bumps {
        let (name, bump_type) = parse_package_bump(input)?;
        map.insert(name, bump_type);
    }

    Ok(map)
}

fn parse_package_bump(input: &str) -> Result<(String, BumpType)> {
    let Some((name, bump_str)) = input.split_once(':') else {
        return Err(CliError::InvalidPackageBumpFormat {
            input: input.to_string(),
        });
    };

    let bump_type = match bump_str.to_lowercase().as_str() {
        "major" => BumpType::Major,
        "minor" => BumpType::Minor,
        "patch" => BumpType::Patch,
        _ => {
            return Err(CliError::InvalidBumpType {
                input: bump_str.to_string(),
            });
        }
    };

    Ok((name.to_string(), bump_type))
}

fn read_description_from_stdin() -> Result<String> {
    let mut buffer = String::new();
    std::io::stdin().read_to_string(&mut buffer)?;
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use changeset_core::BumpType;

    use super::{parse_package_bump, parse_package_bumps};
    use crate::error::CliError;

    #[test]
    fn parse_package_bump_valid_major() {
        let (name, bump) = parse_package_bump("my-package:major").expect("should parse");

        assert_eq!(name, "my-package");
        assert_eq!(bump, BumpType::Major);
    }

    #[test]
    fn parse_package_bump_valid_minor() {
        let (name, bump) = parse_package_bump("my-package:minor").expect("should parse");

        assert_eq!(name, "my-package");
        assert_eq!(bump, BumpType::Minor);
    }

    #[test]
    fn parse_package_bump_valid_patch() {
        let (name, bump) = parse_package_bump("my-package:patch").expect("should parse");

        assert_eq!(name, "my-package");
        assert_eq!(bump, BumpType::Patch);
    }

    #[test]
    fn parse_package_bump_case_insensitive() {
        let (_, bump) = parse_package_bump("package:MAJOR").expect("should parse");
        assert_eq!(bump, BumpType::Major);

        let (_, bump) = parse_package_bump("package:Minor").expect("should parse");
        assert_eq!(bump, BumpType::Minor);

        let (_, bump) = parse_package_bump("package:PATCH").expect("should parse");
        assert_eq!(bump, BumpType::Patch);
    }

    #[test]
    fn parse_package_bump_missing_colon() {
        let result = parse_package_bump("my-package-patch");

        assert!(matches!(
            result,
            Err(CliError::InvalidPackageBumpFormat { input }) if input == "my-package-patch"
        ));
    }

    #[test]
    fn parse_package_bump_invalid_bump_type() {
        let result = parse_package_bump("my-package:huge");

        assert!(matches!(
            result,
            Err(CliError::InvalidBumpType { input }) if input == "huge"
        ));
    }

    #[test]
    fn parse_package_bumps_multiple() {
        let inputs = vec!["a:major".to_string(), "b:minor".to_string()];

        let map = parse_package_bumps(&inputs).expect("should parse");

        assert_eq!(map.get("a"), Some(&BumpType::Major));
        assert_eq!(map.get("b"), Some(&BumpType::Minor));
    }

    #[test]
    fn parse_package_bumps_empty() {
        let map = parse_package_bumps(&[]).expect("should parse");

        assert!(map.is_empty());
    }
}
