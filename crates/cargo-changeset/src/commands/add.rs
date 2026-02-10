use std::io::IsTerminal;

use changeset_core::PackageInfo;
use changeset_workspace::{Workspace, WorkspaceKind, discover_workspace_from_cwd};
use dialoguer::MultiSelect;
use indexmap::IndexSet;

use super::AddArgs;
use crate::error::{CliError, Result};

pub(super) fn run(args: AddArgs) -> Result<()> {
    let workspace = discover_workspace_from_cwd()?;

    if workspace.packages.is_empty() {
        return Err(CliError::EmptyWorkspace(workspace.root));
    }

    let packages = match select_crates(&workspace, &args.crates) {
        Ok(packages) if packages.is_empty() => return Ok(()),
        Ok(packages) => packages,
        Err(CliError::Cancelled) => return Ok(()),
        Err(e) => return Err(e),
    };

    println!();
    println!("Selected {} crate(s):", packages.len());
    for package in &packages {
        println!("  - {} ({})", package.name, package.version);
    }

    Ok(())
}

fn select_crates(workspace: &Workspace, explicit_crates: &[String]) -> Result<Vec<PackageInfo>> {
    if !explicit_crates.is_empty() {
        return resolve_explicit_crates(&workspace.packages, explicit_crates);
    }

    if workspace.kind == WorkspaceKind::SingleCrate {
        let package = workspace
            .packages
            .first()
            .ok_or(CliError::WorkspaceInvariantViolation)?;
        return Ok(select_single_crate(package));
    }

    select_multiple_crates(&workspace.packages)
}

fn resolve_explicit_crates(
    packages: &[PackageInfo],
    crate_names: &[String],
) -> Result<Vec<PackageInfo>> {
    let unique_names: IndexSet<&String> = crate_names.iter().collect();
    let mut selected = Vec::with_capacity(unique_names.len());

    for name in unique_names {
        let package = packages.iter().find(|p| p.name == *name).ok_or_else(|| {
            let available = packages
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            CliError::UnknownCrate {
                name: name.clone(),
                available,
            }
        })?;
        selected.push(package.clone());
    }

    Ok(selected)
}

fn select_single_crate(package: &PackageInfo) -> Vec<PackageInfo> {
    println!("Using crate: {} ({})", package.name, package.version);
    vec![package.clone()]
}

fn select_multiple_crates(packages: &[PackageInfo]) -> Result<Vec<PackageInfo>> {
    if !is_interactive() {
        return Err(CliError::NotATty);
    }

    let items = format_package_items(packages);

    let selection = MultiSelect::new()
        .with_prompt("Select crates to include in changeset")
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

fn is_interactive() -> bool {
    std::env::var("CARGO_CHANGESET_FORCE_TTY").is_ok() || std::io::stdin().is_terminal()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use changeset_core::PackageInfo;
    use semver::Version;

    use super::{format_package_items, resolve_explicit_crates, select_single_crate};
    use crate::error::CliError;

    fn test_package(name: &str, version: &str) -> PackageInfo {
        PackageInfo {
            name: name.to_string(),
            version: Version::parse(version).expect("valid test version"),
            path: PathBuf::from(format!("/test/{name}/Cargo.toml")),
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
    fn select_single_crate_returns_cloned_package() {
        let package = test_package("my-crate", "0.1.0");

        let result = select_single_crate(&package);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "my-crate");
        assert_eq!(result[0].version, Version::new(0, 1, 0));
    }

    #[test]
    fn resolve_explicit_crates_finds_matching_packages() {
        let packages = vec![
            test_package("foo", "1.0.0"),
            test_package("bar", "2.0.0"),
            test_package("baz", "3.0.0"),
        ];
        let names = vec!["bar".to_string(), "foo".to_string()];

        let result = resolve_explicit_crates(&packages, &names).expect("should resolve");

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "bar");
        assert_eq!(result[1].name, "foo");
    }

    #[test]
    fn resolve_explicit_crates_returns_error_for_unknown_crate() {
        let packages = vec![test_package("foo", "1.0.0"), test_package("bar", "2.0.0")];
        let names = vec!["unknown".to_string()];

        let result = resolve_explicit_crates(&packages, &names);

        assert!(matches!(
            result,
            Err(CliError::UnknownCrate { name, available })
                if name == "unknown" && available.contains("foo") && available.contains("bar")
        ));
    }

    #[test]
    fn resolve_explicit_crates_empty_names_returns_empty() {
        let packages = vec![test_package("foo", "1.0.0")];
        let names: Vec<String> = vec![];

        let result = resolve_explicit_crates(&packages, &names).expect("should resolve");

        assert!(result.is_empty());
    }

    #[test]
    fn resolve_explicit_crates_preserves_order() {
        let packages = vec![
            test_package("alpha", "1.0.0"),
            test_package("beta", "2.0.0"),
            test_package("gamma", "3.0.0"),
        ];
        let names = vec!["gamma".to_string(), "alpha".to_string(), "beta".to_string()];

        let result = resolve_explicit_crates(&packages, &names).expect("should resolve");

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].name, "gamma");
        assert_eq!(result[1].name, "alpha");
        assert_eq!(result[2].name, "beta");
    }

    #[test]
    fn resolve_explicit_crates_deduplicates_preserving_first_occurrence() {
        let packages = vec![test_package("foo", "1.0.0"), test_package("bar", "2.0.0")];
        let names = vec![
            "foo".to_string(),
            "bar".to_string(),
            "foo".to_string(),
            "bar".to_string(),
        ];

        let result = resolve_explicit_crates(&packages, &names).expect("should resolve");

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "foo");
        assert_eq!(result[1].name, "bar");
    }

    #[test]
    fn resolve_explicit_crates_fails_fast_on_first_unknown() {
        let packages = vec![test_package("foo", "1.0.0"), test_package("bar", "2.0.0")];
        let names = vec!["foo".to_string(), "unknown".to_string(), "bar".to_string()];

        let result = resolve_explicit_crates(&packages, &names);

        assert!(matches!(
            result,
            Err(CliError::UnknownCrate { name, .. }) if name == "unknown"
        ));
    }

    #[test]
    fn resolve_explicit_crates_with_hyphenated_name() {
        let packages = vec![
            test_package("my-cool-crate", "1.0.0"),
            test_package("another-one", "2.0.0"),
        ];
        let names = vec!["my-cool-crate".to_string()];

        let result = resolve_explicit_crates(&packages, &names).expect("should resolve");

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "my-cool-crate");
    }
}
