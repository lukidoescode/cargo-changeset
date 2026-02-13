mod add;
mod init;
mod release;
mod status;
mod verify;

use std::path::Path;

use changeset_core::{BumpType, ChangeCategory};
use clap::{Args, Subcommand};

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
  1. cargo changeset release --prerelease alpha  → 1.0.0 becomes 1.0.1-alpha.1
  2. cargo changeset release --prerelease        → increments to 1.0.1-alpha.2
  3. cargo changeset release                     → graduates to stable 1.0.1

When graduating from a pre-release, all accumulated changes are combined
into the final changelog entry."
    )]
    Release(ReleaseArgs),
    /// Initialize changeset directory in the project
    Init,
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

    /// Create a pre-release version with the specified tag.
    /// Built-in tags: alpha, beta, rc. Custom tags are also supported
    /// (e.g., --prerelease nightly, --prerelease canary, --prerelease dev).
    /// If tag is omitted on a pre-release version, reuses the current tag.
    /// To graduate a pre-release to stable, run without this flag.
    #[arg(long, value_name = "TAG", num_args = 0..=1, default_missing_value = "")]
    pub prerelease: Option<String>,

    /// Force release without changesets (only valid for pre-release increment)
    #[arg(long, short = 'f')]
    pub force: bool,

    /// Graduate all 0.x packages to 1.0.0
    #[arg(long)]
    pub graduate: bool,
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
            Self::Init => (init::run(start_path), ExecuteResult { quiet: false }),
        }
    }
}
