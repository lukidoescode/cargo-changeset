use std::io::IsTerminal;

use changeset_core::PackageInfo;
use changeset_workspace::{Workspace, WorkspaceKind, discover_workspace_from_cwd};
use dialoguer::MultiSelect;

use crate::error::{CliError, Result};

pub(super) fn run() -> Result<()> {
    let workspace = discover_workspace_from_cwd()?;

    if workspace.packages.is_empty() {
        return Err(CliError::EmptyWorkspace(workspace.root));
    }

    let packages = match select_crates(&workspace) {
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

fn select_crates(workspace: &Workspace) -> Result<Vec<PackageInfo>> {
    if workspace.kind == WorkspaceKind::SingleCrate {
        let package = workspace
            .packages
            .first()
            .ok_or(CliError::WorkspaceInvariantViolation)?;
        return Ok(select_single_crate(package));
    }

    select_multiple_crates(&workspace.packages)
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

    use super::{format_package_items, select_single_crate};

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
}
