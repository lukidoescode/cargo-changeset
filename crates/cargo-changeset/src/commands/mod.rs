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
