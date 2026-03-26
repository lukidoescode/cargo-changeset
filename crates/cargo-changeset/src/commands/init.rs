use std::path::Path;

use changeset_git::DEFAULT_BASE_BRANCH;
use changeset_manifest::InitConfig;
use changeset_operations::operations::{
    InitInput, InitInputBuilder, InitOperation, InitPlan, build_config_from_input,
};
use changeset_operations::providers::{FileSystemManifestWriter, FileSystemProjectProvider};
use changeset_operations::traits::{
    ChangelogSettingsInput, FilteringSettingsInput, GitSettingsInput, InitInteractionProvider,
    ProjectContext, ProjectProvider, VersionSettingsInput,
};
use changeset_project::ProjectKind;

use crate::commands::InitArgs;
use crate::error::Result;
use crate::interaction::{
    TerminalInitInteractionProvider, confirm_proceed, is_terminal_interactive,
};

pub(crate) fn run(args: InitArgs, start_path: &Path) -> Result<()> {
    let project_provider = FileSystemProjectProvider::new();
    let manifest_writer = FileSystemManifestWriter::new();
    let interaction_provider = TerminalInitInteractionProvider::new();

    let project = project_provider.discover_project(start_path)?;
    let (root_config, _) = project_provider.load_configs(&project)?;

    let context = ProjectContext {
        is_single_package: *project.kind() == ProjectKind::SinglePackage,
    };

    let is_interactive = !args.no_interactive && is_terminal_interactive();

    let provider = if is_interactive && !args.defaults {
        Some(&interaction_provider)
    } else {
        None
    };
    let input = build_init_input(&args, provider, context)?;

    let config = build_config_from_input(&input, context);

    let changeset_dir_path = root_config.changeset_dir();
    let full_changeset_dir = project.root().join(changeset_dir_path);
    let dir_exists = full_changeset_dir.exists();
    let gitkeep_exists = full_changeset_dir.join(".gitkeep").exists();

    let metadata_section = match project.kind() {
        ProjectKind::VirtualWorkspace | ProjectKind::WorkspaceWithRoot => {
            changeset_manifest::MetadataSection::Workspace
        }
        ProjectKind::SinglePackage => changeset_manifest::MetadataSection::Package,
    };

    let plan = InitPlan::new(
        full_changeset_dir,
        dir_exists,
        gitkeep_exists,
        metadata_section,
        config,
    );

    print_summary(&plan);

    let skip_confirmation = args.defaults || args.no_interactive || !is_terminal_interactive();
    if !skip_confirmation && !confirm_proceed("Proceed with initialization?")? {
        println!("Aborted.");
        return Ok(());
    }

    let operation = InitOperation::new(project_provider)
        .with_manifest_writer(manifest_writer)
        .with_interaction_provider(interaction_provider);

    let output = operation.execute_plan(start_path, &plan)?;

    println!();
    if output.created_dir() {
        println!(
            "Created changeset directory at '{}'",
            output.changeset_dir().display()
        );
    } else {
        println!(
            "Changeset directory already exists at '{}'",
            output.changeset_dir().display()
        );
    }

    if output.created_gitkeep() {
        println!("Created .gitkeep file");
    }

    if output.wrote_config() {
        if let Some(section) = output.config_location() {
            println!("Wrote configuration to {section} in Cargo.toml");
        }
    }

    Ok(())
}

fn has_any_git_args(args: &InitArgs) -> bool {
    args.commit.is_some()
        || args.tags.is_some()
        || args.keep_changesets.is_some()
        || args.tag_format.is_some()
        || args.base_branch.is_some()
        || args.commit_title_template.is_some()
        || args.changes_in_body.is_some()
}

fn has_any_changelog_args(args: &InitArgs) -> bool {
    args.changelog.is_some()
        || args.comparison_links.is_some()
        || args.comparison_links_template.is_some()
        || args.dependency_bump_changelog_template.is_some()
}

fn has_any_version_args(args: &InitArgs) -> bool {
    args.zero_version_behavior.is_some()
        || args.none_bump_behavior.is_some()
        || args.none_bump_promote_message_template.is_some()
}

fn has_any_filtering_args(args: &InitArgs) -> bool {
    !args.ignored_files.is_empty()
}

fn print_summary(plan: &InitPlan) {
    println!();
    println!("=== Initialization Summary ===");
    println!();

    if plan.dir_exists() {
        println!(
            "Directory: {} (already exists)",
            plan.changeset_dir().display()
        );
    } else {
        println!(
            "Directory: {} (will be created)",
            plan.changeset_dir().display()
        );
    }

    if !plan.gitkeep_exists() {
        println!("  - .gitkeep file will be created");
    }

    if !plan.config().is_empty() {
        println!();
        println!(
            "Configuration to be written to {}:",
            plan.metadata_section()
        );
        print_config_summary(plan.config());
    } else {
        println!();
        println!("No configuration will be written (using defaults).");
    }

    println!();
}

fn print_config_summary(config: &InitConfig) {
    if let Some(commit) = config.commit {
        println!("  commit = {commit}");
    }
    if let Some(tags) = config.tags {
        println!("  tags = {tags}");
    }
    if let Some(keep_changesets) = config.keep_changesets {
        println!("  keep_changesets = {keep_changesets}");
    }
    if let Some(ref tag_format) = config.tag_format {
        println!("  tag_format = \"{}\"", tag_format.as_str());
    }
    if let Some(ref changelog) = config.changelog {
        println!("  changelog = \"{}\"", changelog.as_str());
    }
    if let Some(ref comparison_links) = config.comparison_links {
        println!("  comparison_links = \"{}\"", comparison_links.as_str());
    }
    if let Some(ref zero_version_behavior) = config.zero_version_behavior {
        println!(
            "  zero_version_behavior = \"{}\"",
            zero_version_behavior.as_str()
        );
    }
    if let Some(ref base_branch) = config.base_branch {
        println!("  base_branch = \"{base_branch}\"");
    }
    if let Some(ref none_bump_behavior) = config.none_bump_behavior {
        println!("  none_bump_behavior = \"{}\"", none_bump_behavior.as_str());
    }
    if let Some(ref none_bump_promote_message_template) = config.none_bump_promote_message_template
    {
        println!("  none_bump_promote_message_template = \"{none_bump_promote_message_template}\"");
    }
    if let Some(ref commit_title_template) = config.commit_title_template {
        println!("  commit_title_template = \"{commit_title_template}\"");
    }
    if let Some(changes_in_body) = config.changes_in_body {
        println!("  changes_in_body = {changes_in_body}");
    }
    if let Some(ref comparison_links_template) = config.comparison_links_template {
        println!("  comparison_links_template = \"{comparison_links_template}\"");
    }
    if let Some(ref dependency_bump_changelog_template) = config.dependency_bump_changelog_template
    {
        println!("  dependency_bump_changelog_template = \"{dependency_bump_changelog_template}\"");
    }
    if let Some(ref ignored_files) = config.ignored_files {
        if !ignored_files.is_empty() {
            println!("  ignored_files = {:?}", ignored_files);
        }
    }
}

fn build_init_input(
    args: &InitArgs,
    provider: Option<&impl InitInteractionProvider>,
    context: ProjectContext,
) -> Result<InitInput> {
    let git_config = if has_any_git_args(args) {
        Some(GitSettingsInput {
            commit: args.commit.unwrap_or(true),
            tags: args.tags.unwrap_or(true),
            keep_changesets: args.keep_changesets.unwrap_or(false),
            tag_format: args.tag_format.map(Into::into).unwrap_or_else(|| {
                if context.is_single_package {
                    changeset_manifest::TagFormat::VersionOnly
                } else {
                    changeset_manifest::TagFormat::CratePrefixed
                }
            }),
            base_branch: args
                .base_branch
                .clone()
                .unwrap_or_else(|| String::from(DEFAULT_BASE_BRANCH)),
            commit_title_template: args.commit_title_template.clone(),
            changes_in_body: args.changes_in_body,
        })
    } else if let Some(p) = provider {
        p.configure_git_settings(context)?
    } else {
        None
    };

    let changelog_config = if has_any_changelog_args(args) {
        Some(ChangelogSettingsInput {
            changelog: args.changelog.map(Into::into).unwrap_or_default(),
            comparison_links: args.comparison_links.map(Into::into).unwrap_or_default(),
            comparison_links_template: args.comparison_links_template.clone(),
            dependency_bump_changelog_template: args.dependency_bump_changelog_template.clone(),
        })
    } else if let Some(p) = provider {
        p.configure_changelog_settings(context)?
    } else {
        None
    };

    let version_config = if has_any_version_args(args) {
        Some(VersionSettingsInput {
            zero_version_behavior: args.zero_version_behavior.map(Into::into),
            none_bump_behavior: args.none_bump_behavior.map(Into::into),
            none_bump_promote_message_template: args.none_bump_promote_message_template.clone(),
        })
    } else if let Some(p) = provider {
        p.configure_version_settings()?
    } else {
        None
    };

    let filtering_config = if has_any_filtering_args(args) {
        Some(FilteringSettingsInput {
            ignored_files: args.ignored_files.clone(),
        })
    } else if let Some(p) = provider {
        p.configure_filtering_settings()?
    } else {
        None
    };

    Ok(InitInputBuilder::default()
        .defaults(args.defaults)
        .git_config(git_config)
        .changelog_config(changelog_config)
        .version_config(version_config)
        .filtering_config(filtering_config)
        .build()
        .expect("all fields have defaults"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::NoneBumpBehaviorArg;

    fn default_init_args() -> InitArgs {
        InitArgs {
            defaults: false,
            no_interactive: false,
            commit: None,
            tags: None,
            keep_changesets: None,
            tag_format: None,
            changelog: None,
            comparison_links: None,
            zero_version_behavior: None,
            base_branch: None,
            none_bump_behavior: None,
            none_bump_promote_message_template: None,
            commit_title_template: None,
            changes_in_body: None,
            comparison_links_template: None,
            dependency_bump_changelog_template: None,
            ignored_files: Vec::new(),
        }
    }

    #[test]
    fn has_any_version_args_false_when_no_args() {
        let args = default_init_args();
        assert!(!has_any_version_args(&args));
    }

    #[test]
    fn has_any_version_args_true_when_none_bump_behavior_set() {
        let mut args = default_init_args();
        args.none_bump_behavior = Some(NoneBumpBehaviorArg::Allow);
        assert!(has_any_version_args(&args));
    }

    #[test]
    fn has_any_version_args_true_when_none_bump_promote_message_template_set() {
        let mut args = default_init_args();
        args.none_bump_promote_message_template = Some("custom message".to_string());
        assert!(has_any_version_args(&args));
    }

    #[test]
    fn print_config_summary_includes_none_bump_behavior() {
        let config = changeset_manifest::InitConfig {
            none_bump_behavior: Some(changeset_manifest::NoneBumpBehavior::Disallow),
            ..Default::default()
        };
        print_config_summary(&config);
    }

    #[test]
    fn print_config_summary_includes_none_bump_promote_message_template() {
        let config = changeset_manifest::InitConfig {
            none_bump_behavior: Some(changeset_manifest::NoneBumpBehavior::PromoteToPatch),
            none_bump_promote_message_template: Some("Internal changes".to_string()),
            ..Default::default()
        };
        print_config_summary(&config);
    }

    #[test]
    fn build_init_input_passes_none_bump_behavior() {
        let mut args = default_init_args();
        args.none_bump_behavior = Some(NoneBumpBehaviorArg::Disallow);
        let context = ProjectContext {
            is_single_package: true,
        };

        let input = build_init_input(&args, None::<&TerminalInitInteractionProvider>, context)
            .expect("build_init_input should succeed");

        let version = input
            .version_config()
            .expect("version_config should be Some");
        assert_eq!(
            version.none_bump_behavior,
            Some(changeset_manifest::NoneBumpBehavior::Disallow)
        );
    }

    #[test]
    fn build_init_input_passes_none_bump_promote_message_template() {
        let mut args = default_init_args();
        args.none_bump_promote_message_template = Some("Custom message".to_string());
        let context = ProjectContext {
            is_single_package: true,
        };

        let input = build_init_input(&args, None::<&TerminalInitInteractionProvider>, context)
            .expect("build_init_input should succeed");

        let version = input
            .version_config()
            .expect("version_config should be Some");
        assert_eq!(
            version.none_bump_promote_message_template,
            Some("Custom message".to_string())
        );
    }

    #[test]
    fn build_init_input_no_version_args_returns_none_version_config() {
        let args = default_init_args();
        let context = ProjectContext {
            is_single_package: true,
        };

        let input = build_init_input(&args, None::<&TerminalInitInteractionProvider>, context)
            .expect("build_init_input should succeed");

        assert!(input.version_config().is_none());
    }

    #[test]
    fn has_any_filtering_args_false_when_empty() {
        let args = default_init_args();
        assert!(!has_any_filtering_args(&args));
    }

    #[test]
    fn has_any_filtering_args_true_when_ignored_files_set() {
        let mut args = default_init_args();
        args.ignored_files = vec!["*.lock".to_string()];
        assert!(has_any_filtering_args(&args));
    }

    #[test]
    fn has_any_git_args_false_when_no_args() {
        let args = default_init_args();
        assert!(!has_any_git_args(&args));
    }

    #[test]
    fn has_any_git_args_true_when_commit_title_template_set() {
        let mut args = default_init_args();
        args.commit_title_template = Some("Release {new-version}".to_string());
        assert!(has_any_git_args(&args));
    }

    #[test]
    fn has_any_git_args_true_when_changes_in_body_set() {
        let mut args = default_init_args();
        args.changes_in_body = Some(false);
        assert!(has_any_git_args(&args));
    }

    #[test]
    fn has_any_changelog_args_false_when_no_args() {
        let args = default_init_args();
        assert!(!has_any_changelog_args(&args));
    }

    #[test]
    fn has_any_changelog_args_true_when_comparison_links_template_set() {
        let mut args = default_init_args();
        args.comparison_links_template =
            Some("https://github.com/org/repo/compare/{base}...{target}".to_string());
        assert!(has_any_changelog_args(&args));
    }

    #[test]
    fn has_any_changelog_args_true_when_dependency_bump_changelog_template_set() {
        let mut args = default_init_args();
        args.dependency_bump_changelog_template =
            Some("Updated `{dependency}` to v{version}".to_string());
        assert!(has_any_changelog_args(&args));
    }

    #[test]
    fn build_init_input_passes_commit_title_template() {
        let mut args = default_init_args();
        args.commit_title_template = Some("Release {new-version}".to_string());
        args.commit = Some(true);
        let context = ProjectContext {
            is_single_package: true,
        };

        let input = build_init_input(&args, None::<&TerminalInitInteractionProvider>, context)
            .expect("build_init_input should succeed");

        let git = input.git_config().expect("git_config should be Some");
        assert_eq!(
            git.commit_title_template,
            Some("Release {new-version}".to_string())
        );
    }

    #[test]
    fn build_init_input_passes_changes_in_body() {
        let mut args = default_init_args();
        args.changes_in_body = Some(false);
        args.commit = Some(true);
        let context = ProjectContext {
            is_single_package: true,
        };

        let input = build_init_input(&args, None::<&TerminalInitInteractionProvider>, context)
            .expect("build_init_input should succeed");

        let git = input.git_config().expect("git_config should be Some");
        assert_eq!(git.changes_in_body, Some(false));
    }

    #[test]
    fn build_init_input_passes_comparison_links_template() {
        let mut args = default_init_args();
        args.comparison_links_template =
            Some("https://github.com/org/repo/compare/{base}...{target}".to_string());
        args.changelog = Some(crate::commands::ChangelogLocationArg::Root);
        let context = ProjectContext {
            is_single_package: true,
        };

        let input = build_init_input(&args, None::<&TerminalInitInteractionProvider>, context)
            .expect("build_init_input should succeed");

        let changelog = input
            .changelog_config()
            .expect("changelog_config should be Some");
        assert_eq!(
            changelog.comparison_links_template,
            Some("https://github.com/org/repo/compare/{base}...{target}".to_string())
        );
    }

    #[test]
    fn build_init_input_passes_dependency_bump_changelog_template() {
        let mut args = default_init_args();
        args.dependency_bump_changelog_template =
            Some("Updated `{dependency}` to v{version}".to_string());
        args.changelog = Some(crate::commands::ChangelogLocationArg::Root);
        let context = ProjectContext {
            is_single_package: true,
        };

        let input = build_init_input(&args, None::<&TerminalInitInteractionProvider>, context)
            .expect("build_init_input should succeed");

        let changelog = input
            .changelog_config()
            .expect("changelog_config should be Some");
        assert_eq!(
            changelog.dependency_bump_changelog_template,
            Some("Updated `{dependency}` to v{version}".to_string())
        );
    }

    #[test]
    fn build_init_input_passes_ignored_files() {
        let mut args = default_init_args();
        args.ignored_files = vec!["*.lock".to_string(), "docs/**".to_string()];
        let context = ProjectContext {
            is_single_package: true,
        };

        let input = build_init_input(&args, None::<&TerminalInitInteractionProvider>, context)
            .expect("build_init_input should succeed");

        let filtering = input
            .filtering_config()
            .expect("filtering_config should be Some");
        assert_eq!(
            filtering.ignored_files,
            vec!["*.lock".to_string(), "docs/**".to_string()]
        );
    }

    #[test]
    fn print_config_summary_shows_new_fields() {
        let config = changeset_manifest::InitConfig {
            commit_title_template: Some("Release {new-version}".to_string()),
            changes_in_body: Some(true),
            comparison_links_template: Some(
                "https://github.com/org/repo/compare/{base}...{target}".to_string(),
            ),
            dependency_bump_changelog_template: Some(
                "Updated `{dependency}` to v{version}".to_string(),
            ),
            ignored_files: Some(vec!["*.lock".to_string()]),
            ..Default::default()
        };
        print_config_summary(&config);
    }
}
