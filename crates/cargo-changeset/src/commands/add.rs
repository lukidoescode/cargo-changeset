use std::collections::HashMap;
use std::fs;
use std::io::{IsTerminal, Read as _, Write as _};
use std::path::Path;
use std::process::Command;

use changeset_core::{BumpType, ChangeCategory, Changeset, PackageInfo, PackageRelease};
use changeset_parse::serialize_changeset;
use changeset_project::{
    CargoProject, ProjectKind, discover_project, ensure_changeset_dir, parse_root_config,
};
use dialoguer::{MultiSelect, Select};
use indexmap::IndexSet;

use super::AddArgs;
use crate::error::{CliError, Result};

const MAX_FILENAME_ATTEMPTS: usize = 100;

fn validate_package_bump_args(package_bumps: &[String]) -> Result<()> {
    for input in package_bumps {
        parse_package_bump(input)?;
    }
    Ok(())
}

pub(super) fn run(args: AddArgs, start_path: &Path) -> Result<()> {
    validate_package_bump_args(&args.package_bumps)?;

    let project = discover_project(start_path)?;

    if project.packages.is_empty() {
        return Err(CliError::EmptyProject(project.root));
    }

    let packages = match select_packages(&project, &args) {
        Ok(packages) if packages.is_empty() => return Ok(()),
        Ok(packages) => packages,
        Err(CliError::Cancelled) => return Ok(()),
        Err(e) => return Err(e),
    };

    let releases = collect_releases(&packages, &args)?;

    let category = select_category(&args)?;

    let description = get_description(&args)?;
    let description = description.trim();
    if description.is_empty() {
        return Err(CliError::EmptyDescription);
    }

    let changeset = Changeset {
        summary: description.to_string(),
        releases,
        category,
    };

    let config = parse_root_config(&project)?;
    let changeset_dir = ensure_changeset_dir(&project, &config)?;
    let filename = generate_unique_filename(&changeset_dir)?;
    let file_path = changeset_dir.join(&filename);

    let content = serialize_changeset(&changeset)?;
    fs::write(&file_path, content)?;

    let changeset_dir_display = config.changeset_dir().display();
    println!();
    println!("Created changeset: {changeset_dir_display}/{filename}");
    println!();
    println!("Summary: {description}");
    println!("Category: {category}");
    println!();
    println!("Releases:");
    for release in &changeset.releases {
        println!("  - {}: {:?}", release.name, release.bump_type);
    }

    Ok(())
}

fn select_packages(project: &CargoProject, args: &AddArgs) -> Result<Vec<PackageInfo>> {
    let explicit_packages = collect_explicit_packages(args);

    if !explicit_packages.is_empty() {
        return resolve_explicit_packages(&project.packages, &explicit_packages);
    }

    if project.kind == ProjectKind::SinglePackage {
        let package = project
            .packages
            .first()
            .ok_or(CliError::ProjectInvariantViolation)?;
        return Ok(select_single_package(package));
    }

    select_multiple_packages(&project.packages)
}

fn collect_explicit_packages(args: &AddArgs) -> Vec<String> {
    let mut packages: IndexSet<String> = args.packages.iter().cloned().collect();

    for package_bump in &args.package_bumps {
        if let Some((name, _)) = package_bump.split_once(':') {
            packages.insert(name.to_string());
        }
    }

    packages.into_iter().collect()
}

fn resolve_explicit_packages(
    packages: &[PackageInfo],
    package_names: &[String],
) -> Result<Vec<PackageInfo>> {
    let unique_names: IndexSet<&String> = package_names.iter().collect();
    let mut selected = Vec::with_capacity(unique_names.len());

    for name in unique_names {
        let package = packages.iter().find(|p| p.name == *name).ok_or_else(|| {
            let available = packages
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            CliError::UnknownPackage {
                name: name.clone(),
                available,
            }
        })?;
        selected.push(package.clone());
    }

    Ok(selected)
}

fn select_single_package(package: &PackageInfo) -> Vec<PackageInfo> {
    println!("Using package: {} ({})", package.name, package.version);
    vec![package.clone()]
}

fn select_multiple_packages(packages: &[PackageInfo]) -> Result<Vec<PackageInfo>> {
    if !is_interactive() {
        return Err(CliError::NotATty);
    }

    let items = format_package_items(packages);

    let selection = MultiSelect::new()
        .with_prompt("Select packages to include in changeset")
        .items(&items)
        .interact_opt()
        .map_err(|e| match e {
            dialoguer::Error::IO(io_err) => CliError::Io(io_err),
        })?;

    let Some(indices) = selection else {
        return Err(CliError::Cancelled);
    };

    Ok(indices.into_iter().map(|i| packages[i].clone()).collect())
}

fn format_package_items(packages: &[PackageInfo]) -> Vec<String> {
    packages
        .iter()
        .map(|p| format!("{} ({})", p.name, p.version))
        .collect()
}

fn collect_releases(packages: &[PackageInfo], args: &AddArgs) -> Result<Vec<PackageRelease>> {
    let per_package_bumps = parse_package_bumps(&args.package_bumps)?;

    let mut releases = Vec::with_capacity(packages.len());

    for package in packages {
        let bump_type = if let Some(bump) = per_package_bumps.get(&package.name) {
            *bump
        } else if let Some(bump) = args.bump {
            bump
        } else if is_interactive() {
            select_bump_type(&package.name)?
        } else {
            return Err(CliError::MissingBumpType {
                package_name: package.name.clone(),
            });
        };

        releases.push(PackageRelease {
            name: package.name.clone(),
            bump_type,
        });
    }

    Ok(releases)
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

fn select_bump_type(package_name: &str) -> Result<BumpType> {
    let items = [
        "patch - Bug fixes (backwards compatible)",
        "minor - New features (backwards compatible)",
        "major - Breaking changes",
    ];

    let selection = Select::new()
        .with_prompt(format!("Select bump type for '{package_name}'"))
        .items(items)
        .default(0)
        .interact_opt()
        .map_err(|e| match e {
            dialoguer::Error::IO(io_err) => CliError::Io(io_err),
        })?;

    match selection {
        Some(0) => Ok(BumpType::Patch),
        Some(1) => Ok(BumpType::Minor),
        Some(2) => Ok(BumpType::Major),
        _ => Err(CliError::Cancelled),
    }
}

fn select_category(args: &AddArgs) -> Result<ChangeCategory> {
    if args.category != ChangeCategory::default() || !is_interactive() || has_explicit_args(args) {
        return Ok(args.category);
    }

    let items = [
        "changed - General changes (default)",
        "added - New features",
        "fixed - Bug fixes",
        "deprecated - Deprecated features",
        "removed - Removed features",
        "security - Security fixes",
    ];

    let selection = Select::new()
        .with_prompt("Select change category")
        .items(items)
        .default(0)
        .interact_opt()
        .map_err(|e| match e {
            dialoguer::Error::IO(io_err) => CliError::Io(io_err),
        })?;

    match selection {
        Some(0) => Ok(ChangeCategory::Changed),
        Some(1) => Ok(ChangeCategory::Added),
        Some(2) => Ok(ChangeCategory::Fixed),
        Some(3) => Ok(ChangeCategory::Deprecated),
        Some(4) => Ok(ChangeCategory::Removed),
        Some(5) => Ok(ChangeCategory::Security),
        _ => Err(CliError::Cancelled),
    }
}

fn has_explicit_args(args: &AddArgs) -> bool {
    args.message.is_some() || !args.packages.is_empty() || !args.package_bumps.is_empty()
}

fn get_description(args: &AddArgs) -> Result<String> {
    if let Some(message) = &args.message {
        if message == "-" {
            return read_description_from_stdin();
        }
        return Ok(message.clone());
    }

    if !is_interactive() {
        return Err(CliError::MissingDescription);
    }

    if args.editor {
        get_description_editor()
    } else {
        get_description_terminal()
    }
}

fn read_description_from_stdin() -> Result<String> {
    let mut buffer = String::new();
    std::io::stdin().read_to_string(&mut buffer)?;
    Ok(buffer)
}

fn get_description_terminal() -> Result<String> {
    println!();
    println!("Enter description (press Enter 3 times to finish):");
    println!();

    let mut lines = Vec::new();
    let mut empty_line_count = 0;

    loop {
        let mut line = String::new();
        std::io::stdin().read_line(&mut line)?;

        let trimmed = line.trim_end_matches(['\n', '\r']);

        if trimmed.is_empty() {
            empty_line_count += 1;
            if empty_line_count >= 2 {
                break;
            }
            lines.push(String::new());
        } else {
            empty_line_count = 0;
            lines.push(trimmed.to_string());
        }
    }

    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }

    Ok(lines.join("\n"))
}

fn get_description_editor() -> Result<String> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());

    let mut temp_file = tempfile::NamedTempFile::new()?;
    let template =
        "# Enter your changeset description above.\n# Lines starting with # will be ignored.\n";
    temp_file.write_all(template.as_bytes())?;
    temp_file.flush()?;

    let status = Command::new(&editor)
        .arg(temp_file.path())
        .status()
        .map_err(|source| CliError::EditorFailed { source })?;

    if !status.success() {
        return Err(CliError::EditorFailed {
            source: std::io::Error::other(format!("editor exited with status: {status}")),
        });
    }

    let content = fs::read_to_string(temp_file.path())?;

    let description: String = content
        .lines()
        .filter(|line| !line.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");

    Ok(description)
}

fn generate_unique_filename(changeset_dir: &Path) -> Result<String> {
    for _ in 0..MAX_FILENAME_ATTEMPTS {
        if let Some(name) = petname::petname(3, "-") {
            let filename = format!("{name}.md");

            if !changeset_dir.join(&filename).exists() {
                return Ok(filename);
            }
        }
    }

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    Ok(format!("changeset-{timestamp}.md"))
}

fn is_interactive() -> bool {
    std::env::var("CARGO_CHANGESET_FORCE_TTY").is_ok() || std::io::stdin().is_terminal()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use changeset_core::{BumpType, PackageInfo};
    use semver::Version;

    use super::{
        collect_explicit_packages, format_package_items, parse_package_bump, parse_package_bumps,
        resolve_explicit_packages, select_single_package,
    };
    use crate::commands::AddArgs;
    use crate::error::CliError;

    fn test_package(name: &str, version: &str) -> PackageInfo {
        PackageInfo {
            name: name.to_string(),
            version: Version::parse(version).expect("valid test version"),
            path: PathBuf::from(format!("/test/{name}/Cargo.toml")),
        }
    }

    fn default_args() -> AddArgs {
        AddArgs {
            packages: vec![],
            bump: None,
            package_bumps: vec![],
            category: changeset_core::ChangeCategory::Changed,
            message: None,
            editor: false,
        }
    }

    #[test]
    fn format_package_items_formats_name_and_version() {
        let packages = vec![test_package("foo", "1.0.0"), test_package("bar", "2.1.0")];

        let items = format_package_items(&packages);

        assert_eq!(items, vec!["foo (1.0.0)", "bar (2.1.0)"]);
    }

    #[test]
    fn format_package_items_empty_returns_empty() {
        let items = format_package_items(&[]);

        assert!(items.is_empty());
    }

    #[test]
    fn select_single_package_returns_cloned_package() {
        let package = test_package("my-package", "0.1.0");

        let result = select_single_package(&package);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "my-package");
        assert_eq!(result[0].version, Version::new(0, 1, 0));
    }

    #[test]
    fn resolve_explicit_packages_finds_matching_packages() {
        let packages = vec![
            test_package("foo", "1.0.0"),
            test_package("bar", "2.0.0"),
            test_package("baz", "3.0.0"),
        ];
        let names = vec!["bar".to_string(), "foo".to_string()];

        let result = resolve_explicit_packages(&packages, &names).expect("should resolve");

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "bar");
        assert_eq!(result[1].name, "foo");
    }

    #[test]
    fn resolve_explicit_packages_returns_error_for_unknown_package() {
        let packages = vec![test_package("foo", "1.0.0"), test_package("bar", "2.0.0")];
        let names = vec!["unknown".to_string()];

        let result = resolve_explicit_packages(&packages, &names);

        assert!(matches!(
            result,
            Err(CliError::UnknownPackage { name, available })
                if name == "unknown" && available.contains("foo") && available.contains("bar")
        ));
    }

    #[test]
    fn resolve_explicit_packages_empty_names_returns_empty() {
        let packages = vec![test_package("foo", "1.0.0")];
        let names: Vec<String> = vec![];

        let result = resolve_explicit_packages(&packages, &names).expect("should resolve");

        assert!(result.is_empty());
    }

    #[test]
    fn resolve_explicit_packages_preserves_order() {
        let packages = vec![
            test_package("alpha", "1.0.0"),
            test_package("beta", "2.0.0"),
            test_package("gamma", "3.0.0"),
        ];
        let names = vec!["gamma".to_string(), "alpha".to_string(), "beta".to_string()];

        let result = resolve_explicit_packages(&packages, &names).expect("should resolve");

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].name, "gamma");
        assert_eq!(result[1].name, "alpha");
        assert_eq!(result[2].name, "beta");
    }

    #[test]
    fn resolve_explicit_packages_deduplicates_preserving_first_occurrence() {
        let packages = vec![test_package("foo", "1.0.0"), test_package("bar", "2.0.0")];
        let names = vec![
            "foo".to_string(),
            "bar".to_string(),
            "foo".to_string(),
            "bar".to_string(),
        ];

        let result = resolve_explicit_packages(&packages, &names).expect("should resolve");

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "foo");
        assert_eq!(result[1].name, "bar");
    }

    #[test]
    fn resolve_explicit_packages_fails_fast_on_first_unknown() {
        let packages = vec![test_package("foo", "1.0.0"), test_package("bar", "2.0.0")];
        let names = vec!["foo".to_string(), "unknown".to_string(), "bar".to_string()];

        let result = resolve_explicit_packages(&packages, &names);

        assert!(matches!(
            result,
            Err(CliError::UnknownPackage { name, .. }) if name == "unknown"
        ));
    }

    #[test]
    fn resolve_explicit_packages_with_hyphenated_name() {
        let packages = vec![
            test_package("my-cool-package", "1.0.0"),
            test_package("another-one", "2.0.0"),
        ];
        let names = vec!["my-cool-package".to_string()];

        let result = resolve_explicit_packages(&packages, &names).expect("should resolve");

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "my-cool-package");
    }

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

    #[test]
    fn collect_explicit_packages_from_package_flag() {
        let args = AddArgs {
            packages: vec!["a".to_string(), "b".to_string()],
            ..default_args()
        };

        let packages = collect_explicit_packages(&args);

        assert_eq!(packages.len(), 2);
        assert!(packages.contains(&"a".to_string()));
        assert!(packages.contains(&"b".to_string()));
    }

    #[test]
    fn collect_explicit_packages_from_package_bump_flag() {
        let args = AddArgs {
            package_bumps: vec!["a:major".to_string(), "b:minor".to_string()],
            ..default_args()
        };

        let packages = collect_explicit_packages(&args);

        assert_eq!(packages.len(), 2);
        assert!(packages.contains(&"a".to_string()));
        assert!(packages.contains(&"b".to_string()));
    }

    #[test]
    fn collect_explicit_packages_merges_and_deduplicates() {
        let args = AddArgs {
            packages: vec!["a".to_string(), "c".to_string()],
            package_bumps: vec!["a:major".to_string(), "b:minor".to_string()],
            ..default_args()
        };

        let packages = collect_explicit_packages(&args);

        assert_eq!(packages.len(), 3);
        assert!(packages.contains(&"a".to_string()));
        assert!(packages.contains(&"b".to_string()));
        assert!(packages.contains(&"c".to_string()));
    }

    #[test]
    fn collect_explicit_packages_empty() {
        let args = default_args();

        let packages = collect_explicit_packages(&args);

        assert!(packages.is_empty());
    }
}
