use std::fs;
use std::io::{IsTerminal, Write as _};
use std::process::Command;

use changeset_core::{BumpType, ChangeCategory, PackageInfo};
use changeset_manifest::{ChangelogLocation, ComparisonLinks, TagFormat, ZeroVersionBehavior};
use changeset_operations::Result;
use changeset_operations::traits::{
    BumpSelection, CategorySelection, ChangelogSettingsInput, DescriptionInput, GitSettingsInput,
    InitInteractionProvider, InteractionProvider, PackageSelection, ProjectContext,
    VersionSettingsInput,
};
use dialoguer::{Confirm, MultiSelect, Select};

use crate::error::CliError;

pub struct TerminalInteractionProvider {
    use_editor: bool,
}

impl TerminalInteractionProvider {
    #[must_use]
    pub fn new(use_editor: bool) -> Self {
        Self { use_editor }
    }
}

impl InteractionProvider for TerminalInteractionProvider {
    fn select_packages(&self, available: &[PackageInfo]) -> Result<PackageSelection> {
        if !is_interactive() {
            return Err(cli_to_operation_error(CliError::NotATty));
        }

        let items: Vec<String> = available
            .iter()
            .map(|p| format!("{} ({})", p.name, p.version))
            .collect();

        let selection = MultiSelect::new()
            .with_prompt("Select packages to include in changeset")
            .items(items)
            .interact_opt()
            .map_err(|e| match e {
                dialoguer::Error::IO(io_err) => cli_to_operation_error(CliError::Io(io_err)),
            })?;

        match selection {
            Some(indices) => {
                let packages = indices.into_iter().map(|i| available[i].clone()).collect();
                Ok(PackageSelection::Selected(packages))
            }
            None => Ok(PackageSelection::Cancelled),
        }
    }

    fn select_bump_type(&self, package_name: &str) -> Result<BumpSelection> {
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
                dialoguer::Error::IO(io_err) => cli_to_operation_error(CliError::Io(io_err)),
            })?;

        match selection {
            Some(0) => Ok(BumpSelection::Selected(BumpType::Patch)),
            Some(1) => Ok(BumpSelection::Selected(BumpType::Minor)),
            Some(2) => Ok(BumpSelection::Selected(BumpType::Major)),
            _ => Ok(BumpSelection::Cancelled),
        }
    }

    fn select_category(&self) -> Result<CategorySelection> {
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
                dialoguer::Error::IO(io_err) => cli_to_operation_error(CliError::Io(io_err)),
            })?;

        match selection {
            Some(0) => Ok(CategorySelection::Selected(ChangeCategory::Changed)),
            Some(1) => Ok(CategorySelection::Selected(ChangeCategory::Added)),
            Some(2) => Ok(CategorySelection::Selected(ChangeCategory::Fixed)),
            Some(3) => Ok(CategorySelection::Selected(ChangeCategory::Deprecated)),
            Some(4) => Ok(CategorySelection::Selected(ChangeCategory::Removed)),
            Some(5) => Ok(CategorySelection::Selected(ChangeCategory::Security)),
            _ => Ok(CategorySelection::Cancelled),
        }
    }

    fn get_description(&self) -> Result<DescriptionInput> {
        if self.use_editor {
            get_description_editor().map_err(cli_to_operation_error)
        } else {
            get_description_terminal().map_err(cli_to_operation_error)
        }
    }
}

fn is_interactive() -> bool {
    std::env::var("CARGO_CHANGESET_FORCE_TTY").is_ok() || std::io::stdin().is_terminal()
}

fn cli_to_operation_error(e: CliError) -> changeset_operations::OperationError {
    use changeset_operations::OperationError;

    match e {
        CliError::Io(io) => OperationError::Io(io),
        CliError::NotATty => OperationError::InteractionRequired,
        CliError::EditorFailed { source } => OperationError::Io(source),
        CliError::Core(e) => OperationError::Core(e),
        CliError::Git(e) => OperationError::Git(e),
        CliError::Project(e) => OperationError::Project(e),
        CliError::Operation(e) => e,
        CliError::CurrentDir(io) => OperationError::Io(io),
        CliError::InvalidPackageBumpFormat { .. }
        | CliError::InvalidBumpType { .. }
        | CliError::InvalidPrereleaseTag { .. }
        | CliError::VerificationFailed { .. }
        | CliError::ChangesetDeleted { .. }
        | CliError::InvalidPrereleaseFormat { .. }
        | CliError::PackageNotFound { .. }
        | CliError::CannotGraduatePrerelease { .. }
        | CliError::CannotGraduateStable { .. } => OperationError::Cancelled,
    }
}

fn get_description_terminal() -> std::result::Result<DescriptionInput, CliError> {
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

    Ok(DescriptionInput::Provided(lines.join("\n")))
}

fn get_description_editor() -> std::result::Result<DescriptionInput, CliError> {
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

    Ok(DescriptionInput::Provided(description))
}

pub struct NonInteractiveProvider;

impl InteractionProvider for NonInteractiveProvider {
    fn select_packages(&self, _available: &[PackageInfo]) -> Result<PackageSelection> {
        Err(changeset_operations::OperationError::InteractionRequired)
    }

    fn select_bump_type(&self, package_name: &str) -> Result<BumpSelection> {
        Err(changeset_operations::OperationError::MissingBumpType {
            package_name: package_name.to_string(),
        })
    }

    fn select_category(&self) -> Result<CategorySelection> {
        Ok(CategorySelection::Selected(ChangeCategory::default()))
    }

    fn get_description(&self) -> Result<DescriptionInput> {
        Err(changeset_operations::OperationError::MissingDescription)
    }
}

pub struct TerminalInitInteractionProvider;

impl TerminalInitInteractionProvider {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl InitInteractionProvider for TerminalInitInteractionProvider {
    fn configure_git_settings(&self, context: ProjectContext) -> Result<Option<GitSettingsInput>> {
        if !is_interactive() {
            return Ok(None);
        }

        let configure = Confirm::new()
            .with_prompt("Configure git settings?")
            .default(true)
            .interact_opt()
            .map_err(|e| match e {
                dialoguer::Error::IO(io) => cli_to_operation_error(CliError::Io(io)),
            })?;

        if configure != Some(true) {
            return Ok(None);
        }

        let commit = select_bool("Create git commits on release?", true)?;
        let tags = select_bool("Create git tags on release?", true)?;
        let keep_changesets = select_bool("Keep changeset files after release?", false)?;
        let tag_format = select_tag_format(context.is_single_package)?;

        Ok(Some(GitSettingsInput {
            commit,
            tags,
            keep_changesets,
            tag_format,
        }))
    }

    fn configure_changelog_settings(
        &self,
        context: ProjectContext,
    ) -> Result<Option<ChangelogSettingsInput>> {
        if !is_interactive() {
            return Ok(None);
        }

        let configure = Confirm::new()
            .with_prompt("Configure changelog settings?")
            .default(true)
            .interact_opt()
            .map_err(|e| match e {
                dialoguer::Error::IO(io) => cli_to_operation_error(CliError::Io(io)),
            })?;

        if configure != Some(true) {
            return Ok(None);
        }

        let changelog = if context.is_single_package {
            ChangelogLocation::Root
        } else {
            select_changelog_location()?
        };
        let comparison_links = select_comparison_links()?;

        Ok(Some(ChangelogSettingsInput {
            changelog,
            comparison_links,
        }))
    }

    fn configure_version_settings(&self) -> Result<Option<VersionSettingsInput>> {
        if !is_interactive() {
            return Ok(None);
        }

        let configure = Confirm::new()
            .with_prompt("Configure version settings?")
            .default(true)
            .interact_opt()
            .map_err(|e| match e {
                dialoguer::Error::IO(io) => cli_to_operation_error(CliError::Io(io)),
            })?;

        if configure != Some(true) {
            return Ok(None);
        }

        let zero_version_behavior = select_zero_version_behavior()?;

        Ok(Some(VersionSettingsInput {
            zero_version_behavior,
        }))
    }
}

fn select_bool(prompt: &str, default: bool) -> Result<bool> {
    Confirm::new()
        .with_prompt(prompt)
        .default(default)
        .interact()
        .map_err(|e| match e {
            dialoguer::Error::IO(io) => cli_to_operation_error(CliError::Io(io)),
        })
}

fn select_tag_format(is_single_package: bool) -> Result<TagFormat> {
    let (items, default_idx) = if is_single_package {
        (
            [
                "version-only - Tags like v1.0.0 (default)",
                "crate-prefixed - Tags like crate-name@1.0.0",
            ],
            0,
        )
    } else {
        (
            [
                "version-only - Tags like v1.0.0",
                "crate-prefixed - Tags like crate-name@1.0.0 (default)",
            ],
            1,
        )
    };

    let selection = Select::new()
        .with_prompt("Select tag format")
        .items(items)
        .default(default_idx)
        .interact_opt()
        .map_err(|e| match e {
            dialoguer::Error::IO(io) => cli_to_operation_error(CliError::Io(io)),
        })?;

    match selection {
        Some(0) => Ok(TagFormat::VersionOnly),
        Some(1) => Ok(TagFormat::CratePrefixed),
        _ => {
            if is_single_package {
                Ok(TagFormat::VersionOnly)
            } else {
                Ok(TagFormat::CratePrefixed)
            }
        }
    }
}

fn select_changelog_location() -> Result<ChangelogLocation> {
    let items = [
        "root - Single CHANGELOG.md at project root (default)",
        "per-package - CHANGELOG.md in each package directory",
    ];

    let selection = Select::new()
        .with_prompt("Select changelog location")
        .items(items)
        .default(0)
        .interact_opt()
        .map_err(|e| match e {
            dialoguer::Error::IO(io) => cli_to_operation_error(CliError::Io(io)),
        })?;

    match selection {
        Some(0) => Ok(ChangelogLocation::Root),
        Some(1) => Ok(ChangelogLocation::PerPackage),
        _ => Ok(ChangelogLocation::default()),
    }
}

fn select_comparison_links() -> Result<ComparisonLinks> {
    let items = [
        "auto - Generate links if git remote detected (default)",
        "enabled - Always generate comparison links",
        "disabled - Never generate comparison links",
    ];

    let selection = Select::new()
        .with_prompt("Select comparison links mode")
        .items(items)
        .default(0)
        .interact_opt()
        .map_err(|e| match e {
            dialoguer::Error::IO(io) => cli_to_operation_error(CliError::Io(io)),
        })?;

    match selection {
        Some(0) => Ok(ComparisonLinks::Auto),
        Some(1) => Ok(ComparisonLinks::Enabled),
        Some(2) => Ok(ComparisonLinks::Disabled),
        _ => Ok(ComparisonLinks::default()),
    }
}

fn select_zero_version_behavior() -> Result<ZeroVersionBehavior> {
    let items = [
        "effective-minor - Major bump on 0.x increments minor (default)",
        "auto-promote-on-major - Major bump on 0.x promotes to 1.0.0",
    ];

    let selection = Select::new()
        .with_prompt("Select zero version (0.x.y) behavior")
        .items(items)
        .default(0)
        .interact_opt()
        .map_err(|e| match e {
            dialoguer::Error::IO(io) => cli_to_operation_error(CliError::Io(io)),
        })?;

    match selection {
        Some(0) => Ok(ZeroVersionBehavior::EffectiveMinor),
        Some(1) => Ok(ZeroVersionBehavior::AutoPromoteOnMajor),
        _ => Ok(ZeroVersionBehavior::default()),
    }
}

/// Asks the user for confirmation before proceeding.
///
/// Returns `true` if the user confirms, `false` if they decline or cancel.
///
/// # Errors
///
/// Returns an error if the prompt cannot be displayed (e.g., not a terminal).
pub fn confirm_proceed(prompt: &str) -> crate::error::Result<bool> {
    if !is_interactive() {
        return Err(CliError::NotATty);
    }

    let confirmed = Confirm::new()
        .with_prompt(prompt)
        .default(true)
        .interact_opt()
        .map_err(|e| match e {
            dialoguer::Error::IO(io) => CliError::Io(io),
        })?;

    Ok(confirmed == Some(true))
}

/// Checks if stdin is a TTY or `CARGO_CHANGESET_FORCE_TTY` is set.
///
/// Used to determine if interactive prompts can be shown to the user.
#[must_use]
pub fn is_terminal_interactive() -> bool {
    is_interactive()
}
