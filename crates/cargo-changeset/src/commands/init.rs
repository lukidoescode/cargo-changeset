use std::path::Path;

use changeset_manifest::InitConfig;
use changeset_operations::operations::{
    InitInput, InitOperation, InitPlan, build_config_from_input,
};
use changeset_operations::providers::{FileSystemManifestWriter, FileSystemProjectProvider};
use changeset_operations::traits::{
    ChangelogSettingsInput, GitSettingsInput, ProjectContext, ProjectProvider, VersionSettingsInput,
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
        is_single_package: project.kind == ProjectKind::SinglePackage,
    };

    let is_interactive = !args.no_interactive && is_terminal_interactive();

    let input = if args.defaults {
        build_init_input(&args, context)
    } else if is_interactive {
        build_init_input_interactive(&args, &interaction_provider, context)?
    } else {
        build_init_input(&args, context)
    };

    let config = build_config_from_input(&input, context);

    let changeset_dir_path = root_config.changeset_dir();
    let full_changeset_dir = project.root.join(changeset_dir_path);
    let dir_exists = full_changeset_dir.exists();
    let gitkeep_exists = full_changeset_dir.join(".gitkeep").exists();

    let metadata_section = match project.kind {
        ProjectKind::VirtualWorkspace | ProjectKind::WorkspaceWithRoot => {
            changeset_manifest::MetadataSection::Workspace
        }
        ProjectKind::SinglePackage => changeset_manifest::MetadataSection::Package,
    };

    let plan = InitPlan {
        changeset_dir: full_changeset_dir,
        dir_exists,
        gitkeep_exists,
        metadata_section,
        config,
    };

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
    if output.created_dir {
        println!(
            "Created changeset directory at '{}'",
            output.changeset_dir.display()
        );
    } else {
        println!(
            "Changeset directory already exists at '{}'",
            output.changeset_dir.display()
        );
    }

    if output.created_gitkeep {
        println!("Created .gitkeep file");
    }

    if output.wrote_config {
        if let Some(section) = output.config_location {
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
}

fn has_any_changelog_args(args: &InitArgs) -> bool {
    args.changelog.is_some() || args.comparison_links.is_some()
}

fn has_any_version_args(args: &InitArgs) -> bool {
    args.zero_version_behavior.is_some()
}

fn build_init_input_interactive(
    args: &InitArgs,
    provider: &TerminalInitInteractionProvider,
    context: ProjectContext,
) -> Result<InitInput> {
    use changeset_operations::traits::InitInteractionProvider;

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
        })
    } else {
        provider.configure_git_settings(context)?
    };

    let changelog_config = if has_any_changelog_args(args) {
        Some(ChangelogSettingsInput {
            changelog: args.changelog.map(Into::into).unwrap_or_default(),
            comparison_links: args.comparison_links.map(Into::into).unwrap_or_default(),
        })
    } else {
        provider.configure_changelog_settings(context)?
    };

    let version_config = if has_any_version_args(args) {
        Some(VersionSettingsInput {
            zero_version_behavior: args
                .zero_version_behavior
                .map(Into::into)
                .unwrap_or_default(),
        })
    } else {
        provider.configure_version_settings()?
    };

    Ok(InitInput {
        defaults: false,
        git_config,
        changelog_config,
        version_config,
    })
}

fn print_summary(plan: &InitPlan) {
    println!();
    println!("=== Initialization Summary ===");
    println!();

    if plan.dir_exists {
        println!(
            "Directory: {} (already exists)",
            plan.changeset_dir.display()
        );
    } else {
        println!(
            "Directory: {} (will be created)",
            plan.changeset_dir.display()
        );
    }

    if !plan.gitkeep_exists {
        println!("  - .gitkeep file will be created");
    }

    if !plan.config.is_empty() {
        println!();
        println!("Configuration to be written to {}:", plan.metadata_section);
        print_config_summary(&plan.config);
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
}

fn build_init_input(args: &InitArgs, context: ProjectContext) -> InitInput {
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
        })
    } else {
        None
    };

    let changelog_config = if has_any_changelog_args(args) {
        Some(ChangelogSettingsInput {
            changelog: args.changelog.map(Into::into).unwrap_or_default(),
            comparison_links: args.comparison_links.map(Into::into).unwrap_or_default(),
        })
    } else {
        None
    };

    let version_config = if has_any_version_args(args) {
        Some(VersionSettingsInput {
            zero_version_behavior: args
                .zero_version_behavior
                .map(Into::into)
                .unwrap_or_default(),
        })
    } else {
        None
    };

    InitInput {
        defaults: args.defaults,
        git_config,
        changelog_config,
        version_config,
    }
}
