use std::fs;
use std::io::Write as _;
use std::process::Command;

use dialoguer::{Confirm, Input, MultiSelect, Select};
use strum::{EnumMessage, VariantArray};

use changeset_core::{BumpType, ChangeCategory, PackageInfo};
use changeset_git::DEFAULT_BASE_BRANCH;
use changeset_manifest::{
    ChangelogLocation, ComparisonLinks, NoneBumpBehavior, TagFormat, ZeroVersionBehavior,
};
use changeset_operations::Result;
use changeset_operations::traits::{
    AdditionalPackageField, BumpSelection, CategorySelection, ChangelogSettingsInput,
    DescriptionInput, FilteringSettingsInput, GitSettingsInput, InitInteractionProvider,
    InteractionProvider, PackageSelection, ProjectContext, VersionSettingsInput,
};

use crate::commands::{cli_error_to_operation_error, dialoguer_to_operation_error};
use crate::environment::is_interactive;
use crate::error::{CliError, Result as CliResult};

#[derive(Clone, Copy, VariantArray, EnumMessage)]
pub(crate) enum BumpTypeSelectionOption {
    #[strum(message = "patch - Bug fixes (backwards compatible)")]
    Patch,
    #[strum(message = "minor - New features (backwards compatible)")]
    Minor,
    #[strum(message = "major - Breaking changes")]
    Major,
    #[strum(message = "none - No version bump (internal changes only)")]
    None,
}

impl From<BumpTypeSelectionOption> for BumpType {
    fn from(opt: BumpTypeSelectionOption) -> Self {
        match opt {
            BumpTypeSelectionOption::Patch => Self::Patch,
            BumpTypeSelectionOption::Minor => Self::Minor,
            BumpTypeSelectionOption::Major => Self::Major,
            BumpTypeSelectionOption::None => Self::None,
        }
    }
}

#[derive(Clone, Copy, VariantArray, EnumMessage)]
pub(crate) enum ChangeCategorySelectionOption {
    #[strum(message = "changed - General changes (default)")]
    Changed,
    #[strum(message = "added - New features")]
    Added,
    #[strum(message = "fixed - Bug fixes")]
    Fixed,
    #[strum(message = "deprecated - Deprecated features")]
    Deprecated,
    #[strum(message = "removed - Removed features")]
    Removed,
    #[strum(message = "security - Security fixes")]
    Security,
}

impl From<ChangeCategorySelectionOption> for ChangeCategory {
    fn from(opt: ChangeCategorySelectionOption) -> Self {
        match opt {
            ChangeCategorySelectionOption::Changed => Self::Changed,
            ChangeCategorySelectionOption::Added => Self::Added,
            ChangeCategorySelectionOption::Fixed => Self::Fixed,
            ChangeCategorySelectionOption::Deprecated => Self::Deprecated,
            ChangeCategorySelectionOption::Removed => Self::Removed,
            ChangeCategorySelectionOption::Security => Self::Security,
        }
    }
}

#[derive(Clone, Copy, VariantArray, EnumMessage)]
pub(crate) enum ChangelogLocationSelectionOption {
    #[strum(message = "root - Single CHANGELOG.md at project root (default)")]
    Root,
    #[strum(message = "per-package - CHANGELOG.md in each package directory")]
    PerPackage,
}

impl From<ChangelogLocationSelectionOption> for ChangelogLocation {
    fn from(opt: ChangelogLocationSelectionOption) -> Self {
        match opt {
            ChangelogLocationSelectionOption::Root => Self::Root,
            ChangelogLocationSelectionOption::PerPackage => Self::PerPackage,
        }
    }
}

#[derive(Clone, Copy, VariantArray, EnumMessage)]
pub(crate) enum ComparisonLinksSelectionOption {
    #[strum(message = "auto - Generate links if git remote detected (default)")]
    Auto,
    #[strum(message = "enabled - Always generate comparison links")]
    Enabled,
    #[strum(message = "disabled - Never generate comparison links")]
    Disabled,
}

impl From<ComparisonLinksSelectionOption> for ComparisonLinks {
    fn from(opt: ComparisonLinksSelectionOption) -> Self {
        match opt {
            ComparisonLinksSelectionOption::Auto => Self::Auto,
            ComparisonLinksSelectionOption::Enabled => Self::Enabled,
            ComparisonLinksSelectionOption::Disabled => Self::Disabled,
        }
    }
}

#[derive(Clone, Copy, VariantArray, EnumMessage)]
pub(crate) enum ZeroVersionBehaviorSelectionOption {
    #[strum(message = "effective-minor - Major bump on 0.x increments minor (default)")]
    EffectiveMinor,
    #[strum(message = "auto-promote-on-major - Major bump on 0.x promotes to 1.0.0")]
    AutoPromoteOnMajor,
}

impl From<ZeroVersionBehaviorSelectionOption> for ZeroVersionBehavior {
    fn from(opt: ZeroVersionBehaviorSelectionOption) -> Self {
        match opt {
            ZeroVersionBehaviorSelectionOption::EffectiveMinor => Self::EffectiveMinor,
            ZeroVersionBehaviorSelectionOption::AutoPromoteOnMajor => Self::AutoPromoteOnMajor,
        }
    }
}

#[derive(Clone, Copy, VariantArray, EnumMessage)]
pub(crate) enum NoneBumpBehaviorSelectionOption {
    #[strum(message = "promote-to-patch - Treat none bumps as patch releases (default)")]
    PromoteToPatch,
    #[strum(message = "allow - Allow none bumps without version change")]
    Allow,
    #[strum(message = "disallow - Reject changesets with none bump type")]
    Disallow,
}

impl From<NoneBumpBehaviorSelectionOption> for NoneBumpBehavior {
    fn from(opt: NoneBumpBehaviorSelectionOption) -> Self {
        match opt {
            NoneBumpBehaviorSelectionOption::PromoteToPatch => Self::PromoteToPatch,
            NoneBumpBehaviorSelectionOption::Allow => Self::Allow,
            NoneBumpBehaviorSelectionOption::Disallow => Self::Disallow,
        }
    }
}

#[derive(Clone, Copy, VariantArray, EnumMessage)]
pub(crate) enum AdditionalPackageFieldSelectionOption {
    #[strum(message = "path")]
    Path,
    #[strum(message = "influence patterns")]
    Influence,
    #[strum(message = "manifest file path")]
    ManifestFilePath,
    #[strum(message = "manifest format")]
    ManifestFormat,
    #[strum(message = "manifest version path")]
    ManifestVersionFieldPath,
    #[strum(message = "Done")]
    Done,
}

impl From<AdditionalPackageFieldSelectionOption> for Option<AdditionalPackageField> {
    fn from(opt: AdditionalPackageFieldSelectionOption) -> Self {
        match opt {
            AdditionalPackageFieldSelectionOption::Path => Some(AdditionalPackageField::Path),
            AdditionalPackageFieldSelectionOption::Influence => {
                Some(AdditionalPackageField::Influence)
            }
            AdditionalPackageFieldSelectionOption::ManifestFilePath => {
                Some(AdditionalPackageField::ManifestFilePath)
            }
            AdditionalPackageFieldSelectionOption::ManifestFormat => {
                Some(AdditionalPackageField::ManifestFormat)
            }
            AdditionalPackageFieldSelectionOption::ManifestVersionFieldPath => {
                Some(AdditionalPackageField::ManifestVersionFieldPath)
            }
            AdditionalPackageFieldSelectionOption::Done => None,
        }
    }
}

pub(crate) struct TerminalInteractionProvider {
    use_editor: bool,
}

impl TerminalInteractionProvider {
    #[must_use]
    pub(crate) fn new(use_editor: bool) -> Self {
        Self { use_editor }
    }
}

impl InteractionProvider for TerminalInteractionProvider {
    fn select_packages(
        &self,
        available: &[PackageInfo],
        display_labels: Option<&[String]>,
    ) -> Result<PackageSelection> {
        if !is_interactive() {
            return Err(changeset_operations::OperationError::InteractionRequired);
        }

        let default_labels: Vec<String>;
        let items: &[String] = match display_labels {
            Some(labels) => labels,
            None => {
                default_labels = available
                    .iter()
                    .map(|p| format!("{} ({})", p.name(), p.version()))
                    .collect();
                &default_labels
            }
        };

        let selection = MultiSelect::new()
            .with_prompt("Select packages to include in changeset")
            .items(items)
            .interact_opt()
            .map_err(dialoguer_to_operation_error)?;

        match selection {
            Some(indices) => {
                let packages = indices.into_iter().map(|i| available[i].clone()).collect();
                Ok(PackageSelection::Selected(packages))
            }
            None => Ok(PackageSelection::Cancelled),
        }
    }

    fn select_bump_type(&self, package_name: &str) -> Result<BumpSelection> {
        let selection = select_variant::<BumpTypeSelectionOption>(
            &format!("Select bump type for '{package_name}'"),
            0,
        )
        .map_err(cli_error_to_operation_error)?;
        Ok(match selection {
            Some(opt) => BumpSelection::Selected(opt.into()),
            None => BumpSelection::Cancelled,
        })
    }

    fn select_category(&self) -> Result<CategorySelection> {
        let selection =
            select_variant::<ChangeCategorySelectionOption>("Select change category", 0)
                .map_err(cli_error_to_operation_error)?;
        Ok(match selection {
            Some(opt) => CategorySelection::Selected(opt.into()),
            None => CategorySelection::Cancelled,
        })
    }

    fn get_description(&self) -> Result<DescriptionInput> {
        if self.use_editor {
            get_description_editor().map_err(cli_error_to_operation_error)
        } else {
            get_description_terminal().map_err(cli_error_to_operation_error)
        }
    }
}

pub(crate) struct NonInteractiveProvider;

impl InteractionProvider for NonInteractiveProvider {
    fn select_packages(
        &self,
        _available: &[PackageInfo],
        _display_labels: Option<&[String]>,
    ) -> Result<PackageSelection> {
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

pub(crate) struct MultiValuePromptConfig<'a> {
    pub(crate) intro: &'a str,
    pub(crate) first_prompt: &'a str,
    pub(crate) additional_prompt: &'a str,
    pub(crate) first_default: Option<String>,
}

#[derive(Default)]
pub(crate) struct TerminalInitInteractionProvider;

impl TerminalInitInteractionProvider {
    #[must_use]
    pub(crate) fn new() -> Self {
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
            .map_err(dialoguer_to_operation_error)?;

        if configure != Some(true) {
            return Ok(None);
        }

        let commit = select_bool("Create git commits on release?", true)
            .map_err(cli_error_to_operation_error)?;
        let tags = select_bool("Create git tags on release?", true)
            .map_err(cli_error_to_operation_error)?;
        let keep_changesets = select_bool("Keep changeset files after release?", false)
            .map_err(cli_error_to_operation_error)?;
        let tag_format =
            select_tag_format(context.is_single_package).map_err(cli_error_to_operation_error)?;
        let base_branch = prompt_base_branch().map_err(cli_error_to_operation_error)?;

        let (commit_title_template, changes_in_body) = if commit {
            let template = prompt_commit_title_template().map_err(cli_error_to_operation_error)?;
            let body = select_bool("Include version details in commit body?", true)
                .map_err(cli_error_to_operation_error)?;
            (Some(template), Some(body))
        } else {
            (None, None)
        };

        Ok(Some(GitSettingsInput {
            commit,
            tags,
            keep_changesets,
            tag_format,
            base_branch,
            commit_title_template,
            changes_in_body,
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
            .map_err(dialoguer_to_operation_error)?;

        if configure != Some(true) {
            return Ok(None);
        }

        let changelog = if context.is_single_package {
            ChangelogLocation::Root
        } else {
            select_changelog_location().map_err(cli_error_to_operation_error)?
        };
        let comparison_links = select_comparison_links().map_err(cli_error_to_operation_error)?;

        let comparison_links_template = if comparison_links != ComparisonLinks::Disabled {
            let template =
                prompt_comparison_links_template().map_err(cli_error_to_operation_error)?;
            if template.is_empty() {
                None
            } else {
                Some(template)
            }
        } else {
            None
        };

        let dep_template =
            prompt_dependency_bump_changelog_template().map_err(cli_error_to_operation_error)?;
        let dependency_bump_changelog_template =
            if dep_template == "Updated dependency `{dependency}` to v{version}" {
                None
            } else {
                Some(dep_template)
            };

        Ok(Some(ChangelogSettingsInput {
            changelog,
            comparison_links,
            comparison_links_template,
            dependency_bump_changelog_template,
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
            .map_err(dialoguer_to_operation_error)?;

        if configure != Some(true) {
            return Ok(None);
        }

        let zero_version_behavior =
            select_zero_version_behavior().map_err(cli_error_to_operation_error)?;
        let none_bump_behavior =
            select_none_bump_behavior().map_err(cli_error_to_operation_error)?;
        let none_bump_promote_message_template =
            if none_bump_behavior == NoneBumpBehavior::PromoteToPatch {
                Some(
                    prompt_none_bump_promote_message_template()
                        .map_err(cli_error_to_operation_error)?,
                )
            } else {
                None
            };

        Ok(Some(VersionSettingsInput {
            zero_version_behavior: Some(zero_version_behavior),
            none_bump_behavior: Some(none_bump_behavior),
            none_bump_promote_message_template,
        }))
    }

    fn configure_filtering_settings(&self) -> Result<Option<FilteringSettingsInput>> {
        if !is_interactive() {
            return Ok(None);
        }

        let configure = Confirm::new()
            .with_prompt("Configure file filtering?")
            .default(false)
            .interact_opt()
            .map_err(dialoguer_to_operation_error)?;

        if configure != Some(true) {
            return Ok(None);
        }

        let ignored_files = prompt_ignored_files_loop().map_err(cli_error_to_operation_error)?;

        if ignored_files.is_empty() {
            return Ok(None);
        }

        Ok(Some(FilteringSettingsInput { ignored_files }))
    }
}

pub(crate) fn confirm_proceed(prompt: &str) -> CliResult<bool> {
    if !is_interactive() {
        return Err(CliError::NotATty);
    }

    let confirmed = Confirm::new()
        .with_prompt(prompt)
        .default(true)
        .interact_opt()?;

    Ok(confirmed == Some(true))
}

pub(crate) fn prompt_multi_value(config: &MultiValuePromptConfig<'_>) -> CliResult<Vec<String>> {
    let mut values = Vec::new();
    println!("{}", config.intro);

    let mut first_input = Input::<String>::new()
        .with_prompt(config.first_prompt)
        .allow_empty(true);
    if let Some(ref default) = config.first_default {
        first_input = first_input.default(default.clone());
    }
    let first = first_input.interact_text()?;
    let first = first.trim().to_string();
    if first.is_empty() {
        return Ok(values);
    }
    values.push(first);

    loop {
        let s: String = Input::new()
            .with_prompt(config.additional_prompt)
            .allow_empty(true)
            .interact_text()?;
        let s = s.trim().to_string();
        if s.is_empty() {
            break;
        }
        values.push(s);
    }
    Ok(values)
}

pub(crate) fn select_variant<T>(prompt: &str, default: usize) -> CliResult<Option<T>>
where
    T: Copy + VariantArray + EnumMessage,
{
    let variants = T::VARIANTS;
    let items: Vec<&str> = variants
        .iter()
        .map(|v| v.get_message().unwrap_or(""))
        .collect();
    let selection = Select::new()
        .with_prompt(prompt)
        .items(&items)
        .default(default)
        .interact_opt()?;
    Ok(selection.map(|idx| variants[idx]))
}

pub(crate) fn select_from_options<T: Copy>(
    prompt: &str,
    options: &[(T, &str)],
    default: usize,
) -> CliResult<Option<T>> {
    let items: Vec<&str> = options.iter().map(|(_, label)| *label).collect();
    let selection = Select::new()
        .with_prompt(prompt)
        .items(&items)
        .default(default)
        .interact_opt()?;
    Ok(selection.map(|idx| options[idx].0))
}

fn get_description_terminal() -> CliResult<DescriptionInput> {
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

fn get_description_editor() -> CliResult<DescriptionInput> {
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

fn prompt_base_branch() -> CliResult<String> {
    Ok(Input::new()
        .with_prompt("Default base branch for git comparisons")
        .default(DEFAULT_BASE_BRANCH.to_string())
        .interact_text()?)
}

fn select_bool(prompt: &str, default: bool) -> CliResult<bool> {
    Ok(Confirm::new()
        .with_prompt(prompt)
        .default(default)
        .interact()?)
}

fn select_tag_format(is_single_package: bool) -> CliResult<TagFormat> {
    let (options, default_idx): (&[(TagFormat, &str)], usize) = if is_single_package {
        (
            &[
                (
                    TagFormat::VersionOnly,
                    "version-only - Tags like v1.0.0 (default)",
                ),
                (
                    TagFormat::CratePrefixed,
                    "crate-prefixed - Tags like crate-name@1.0.0",
                ),
            ],
            0,
        )
    } else {
        (
            &[
                (TagFormat::VersionOnly, "version-only - Tags like v1.0.0"),
                (
                    TagFormat::CratePrefixed,
                    "crate-prefixed - Tags like crate-name@1.0.0 (default)",
                ),
            ],
            1,
        )
    };

    let selection = select_from_options("Select tag format", options, default_idx)?;
    Ok(selection.unwrap_or(options[default_idx].0))
}

fn select_changelog_location() -> CliResult<ChangelogLocation> {
    let selection =
        select_variant::<ChangelogLocationSelectionOption>("Select changelog location", 0)?;
    Ok(selection.map_or_else(ChangelogLocation::default, Into::into))
}

fn select_comparison_links() -> CliResult<ComparisonLinks> {
    let selection =
        select_variant::<ComparisonLinksSelectionOption>("Select comparison links mode", 0)?;
    Ok(selection.map_or_else(ComparisonLinks::default, Into::into))
}

fn select_zero_version_behavior() -> CliResult<ZeroVersionBehavior> {
    let selection = select_variant::<ZeroVersionBehaviorSelectionOption>(
        "Select zero version (0.x.y) behavior",
        0,
    )?;
    Ok(selection.map_or_else(ZeroVersionBehavior::default, Into::into))
}

fn select_none_bump_behavior() -> CliResult<NoneBumpBehavior> {
    let selection =
        select_variant::<NoneBumpBehaviorSelectionOption>("Select none bump behavior", 0)?;
    Ok(selection.map_or_else(NoneBumpBehavior::default, Into::into))
}

fn prompt_commit_title_template() -> CliResult<String> {
    Ok(Input::new()
        .with_prompt("Commit title template (placeholder: {new-version})")
        .default("{new-version}".to_string())
        .interact_text()?)
}

fn prompt_comparison_links_template() -> CliResult<String> {
    Ok(Input::new()
        .with_prompt(
            "Comparison links template (empty=auto-detect, placeholders: {repository}, {base}, {target})",
        )
        .default(String::new())
        .allow_empty(true)
        .interact_text()?)
}

fn prompt_dependency_bump_changelog_template() -> CliResult<String> {
    Ok(Input::new()
        .with_prompt("Dependency bump changelog template (placeholders: {dependency}, {version})")
        .default("Updated dependency `{dependency}` to v{version}".to_string())
        .interact_text()?)
}

fn prompt_ignored_files_loop() -> CliResult<Vec<String>> {
    prompt_multi_value(&MultiValuePromptConfig {
        intro: "Enter file patterns to exclude from change detection \
                (one per line, empty line to finish):",
        first_prompt: "Ignore pattern",
        additional_prompt: "Additional pattern",
        first_default: None,
    })
}

fn prompt_none_bump_promote_message_template() -> CliResult<String> {
    Ok(Input::new()
        .with_prompt("Changelog message template for promoted none bumps")
        .default("Internal architectural changes".to_string())
        .interact_text()?)
}
