mod add;
mod init;
mod manage;
mod release;
mod status;
mod verify;

use std::path::Path;

use changeset_core::{BumpType, ChangeCategory};
use changeset_manifest::{ChangelogLocation, ComparisonLinks, TagFormat, ZeroVersionBehavior};
use clap::{Args, Subcommand, ValueEnum};

use crate::error::Result;

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Add a new changeset
    Add(AddArgs),
    /// Verify changeset coverage for changed packages
    Verify(VerifyArgs),
    /// Show pending changesets and projected version bumps
    Status,
    /// Calculate version bumps and prepare releases based on pending changesets
    #[command(
        verbatim_doc_comment,
        after_long_help = "\
Pre-release workflow:
  1. cargo changeset release --prerelease alpha      → All packages get alpha tag
  2. cargo changeset release --prerelease foo:alpha  → Only foo gets alpha tag
  3. cargo changeset release                         → Graduates prereleases to stable

Graduation (0.x to 1.0.0):
  - cargo changeset release --graduate foo --graduate bar
  - Or configure in .changeset/graduation.toml

Per-package configuration can also be set via:
  - .changeset/pre-release.toml
  - .changeset/graduation.toml
Use 'cargo changeset manage' to configure these files."
    )]
    Release(ReleaseArgs),
    /// Initialize changeset directory in the project
    Init(InitArgs),
    /// Manage release configuration files
    Manage(ManageArgs),
}

#[derive(Args)]
pub(crate) struct InitArgs {
    /// Use default configuration values without prompts
    #[arg(long)]
    pub defaults: bool,

    /// Disable interactive prompts (use only CLI-provided values)
    #[arg(long)]
    pub no_interactive: bool,

    /// Create git commits on release (default: true)
    #[arg(long)]
    pub commit: Option<bool>,

    /// Create git tags on release (default: true)
    #[arg(long)]
    pub tags: Option<bool>,

    /// Keep changeset files after release (default: false)
    #[arg(long)]
    pub keep_changesets: Option<bool>,

    /// Tag format: "version-only" or "crate-prefixed" (default: version-only)
    #[arg(long, value_name = "FORMAT")]
    pub tag_format: Option<TagFormatArg>,

    /// Changelog location: "root" or "per-package" (default: root)
    #[arg(long, value_name = "LOCATION")]
    pub changelog: Option<ChangelogLocationArg>,

    /// Comparison links: "auto", "enabled", or "disabled" (default: auto)
    #[arg(long, value_name = "MODE")]
    pub comparison_links: Option<ComparisonLinksArg>,

    /// Zero version behavior: "effective-minor" or "auto-promote-on-major" (default: effective-minor)
    #[arg(long, value_name = "BEHAVIOR")]
    pub zero_version_behavior: Option<ZeroVersionBehaviorArg>,
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum TagFormatArg {
    VersionOnly,
    CratePrefixed,
}

impl From<TagFormatArg> for TagFormat {
    fn from(arg: TagFormatArg) -> Self {
        match arg {
            TagFormatArg::VersionOnly => Self::VersionOnly,
            TagFormatArg::CratePrefixed => Self::CratePrefixed,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum ChangelogLocationArg {
    Root,
    PerPackage,
}

impl From<ChangelogLocationArg> for ChangelogLocation {
    fn from(arg: ChangelogLocationArg) -> Self {
        match arg {
            ChangelogLocationArg::Root => Self::Root,
            ChangelogLocationArg::PerPackage => Self::PerPackage,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum ComparisonLinksArg {
    Auto,
    Enabled,
    Disabled,
}

impl From<ComparisonLinksArg> for ComparisonLinks {
    fn from(arg: ComparisonLinksArg) -> Self {
        match arg {
            ComparisonLinksArg::Auto => Self::Auto,
            ComparisonLinksArg::Enabled => Self::Enabled,
            ComparisonLinksArg::Disabled => Self::Disabled,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum ZeroVersionBehaviorArg {
    EffectiveMinor,
    AutoPromoteOnMajor,
}

impl From<ZeroVersionBehaviorArg> for ZeroVersionBehavior {
    fn from(arg: ZeroVersionBehaviorArg) -> Self {
        match arg {
            ZeroVersionBehaviorArg::EffectiveMinor => Self::EffectiveMinor,
            ZeroVersionBehaviorArg::AutoPromoteOnMajor => Self::AutoPromoteOnMajor,
        }
    }
}

#[derive(Args)]
pub(crate) struct AddArgs {
    /// Package(s) to include in the changeset (skips interactive selection)
    #[arg(long = "package", short = 'p', value_name = "NAME")]
    pub packages: Vec<String>,

    /// Bump type for all packages (major, minor, patch)
    #[arg(long, short = 'b', value_enum)]
    pub bump: Option<BumpType>,

    /// Per-package bump type: "package-name:bump-type"
    #[arg(long = "package-bump", value_name = "NAME:TYPE")]
    pub package_bumps: Vec<String>,

    /// Change category (defaults to "changed")
    #[arg(long, short = 'c', value_enum, default_value = "changed")]
    pub category: ChangeCategory,

    /// Description (use "-" to read from stdin)
    #[arg(long, short = 'm')]
    pub message: Option<String>,

    /// Open external editor ($EDITOR) for description input
    #[arg(long)]
    pub editor: bool,
}

#[derive(Args)]
pub(crate) struct VerifyArgs {
    /// Base branch to compare against
    #[arg(long, default_value = "main")]
    pub base: String,

    /// Head ref to compare (defaults to HEAD)
    #[arg(long)]
    pub head: Option<String>,

    /// Suppress all output (exit code only, for CI)
    #[arg(long, short = 'q')]
    pub quiet: bool,

    /// Allow deleted changeset files (not recommended)
    #[arg(long, short = 'd')]
    pub allow_deleted_changesets: bool,
}

#[derive(Args)]
pub(crate) struct ReleaseArgs {
    /// Preview changes without modifying any files
    #[arg(long)]
    pub dry_run: bool,

    /// Convert inherited versions (version.workspace = true) to explicit versions
    #[arg(long)]
    pub convert: bool,

    /// Skip all git operations (commit and tags); allows dirty working tree
    #[arg(long)]
    pub no_commit: bool,

    /// Skip creating git tags
    #[arg(long)]
    pub no_tags: bool,

    /// Keep changeset files after release (do not delete them)
    #[arg(long)]
    pub keep_changesets: bool,

    /// Create pre-release for specific crate(s). Format: "crate:tag" or just "tag".
    /// Can be specified multiple times. If no crate specified, applies to all.
    /// Built-in tags: alpha, beta, rc. Custom tags are also supported.
    /// If tag is omitted on a pre-release version, reuses the current tag.
    /// To graduate a pre-release to stable, run without this flag.
    #[arg(long, value_name = "CRATE:TAG", num_args = 0..=1, default_missing_value = "")]
    pub prerelease: Vec<String>,

    /// Force release without changesets (only valid for pre-release increment)
    #[arg(long, short = 'f')]
    pub force: bool,

    /// Graduate specific 0.x crate(s) to 1.0.0.
    /// In workspace: specify which crates to graduate.
    /// For single package: graduates the package (no value needed).
    /// Can be specified multiple times.
    #[arg(long, value_name = "CRATE", num_args = 0..=1, default_missing_value = "")]
    pub graduate: Vec<String>,
}

#[derive(Args)]
pub(crate) struct ManageArgs {
    #[command(subcommand)]
    pub command: ManageCommand,
}

#[derive(Subcommand)]
pub(crate) enum ManageCommand {
    /// Manage active pre-releases (.changeset/pre-release.toml)
    #[command(name = "pre-release")]
    Prerelease(ManagePrereleaseArgs),

    /// Manage graduation queue (.changeset/graduation.toml)
    Graduation(ManageGraduationArgs),
}

#[derive(Args)]
pub(crate) struct ManagePrereleaseArgs {
    /// Add crate to pre-release (format: crate:tag)
    #[arg(long, value_name = "CRATE:TAG")]
    pub add: Vec<String>,

    /// Remove crate from pre-release
    #[arg(long, value_name = "CRATE")]
    pub remove: Vec<String>,

    /// Move crate from pre-release to graduation queue
    /// (only valid if crate is NOT currently in prerelease version)
    #[arg(long, value_name = "CRATE")]
    pub graduate: Vec<String>,

    /// List current pre-release configuration
    #[arg(long, short)]
    pub list: bool,
}

#[derive(Args)]
pub(crate) struct ManageGraduationArgs {
    /// Add crate to graduation queue (must be 0.x version)
    #[arg(long, value_name = "CRATE")]
    pub add: Vec<String>,

    /// Remove crate from graduation queue
    #[arg(long, value_name = "CRATE")]
    pub remove: Vec<String>,

    /// List crates marked for graduation
    #[arg(long, short)]
    pub list: bool,
}

pub(crate) struct ExecuteResult {
    pub quiet: bool,
}

impl Commands {
    pub(crate) fn execute(self, start_path: &Path) -> (Result<()>, ExecuteResult) {
        match self {
            Self::Add(args) => (add::run(args, start_path), ExecuteResult { quiet: false }),
            Self::Verify(args) => {
                let quiet = args.quiet;
                (verify::run(args, start_path), ExecuteResult { quiet })
            }
            Self::Status => (status::run(start_path), ExecuteResult { quiet: false }),
            Self::Release(args) => (
                release::run(args, start_path),
                ExecuteResult { quiet: false },
            ),
            Self::Init(args) => (init::run(args, start_path), ExecuteResult { quiet: false }),
            Self::Manage(args) => (
                manage::run(args, start_path),
                ExecuteResult { quiet: false },
            ),
        }
    }
}
