mod add;
mod verify;

use changeset_core::{BumpType, ChangeCategory};
use clap::{Args, Subcommand};

use crate::error::Result;

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Add a new changeset
    Add(AddArgs),
    /// Verify changeset coverage for changed packages
    Verify(VerifyArgs),
    /// Show status of changesets
    Status,
    /// Bump versions based on changesets
    Version,
    /// Initialize changeset configuration
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

    /// Show detailed output
    #[arg(long, short)]
    pub verbose: bool,
}

impl Commands {
    pub(crate) fn execute(self) -> Result<()> {
        match self {
            Self::Add(args) => add::run(args),
            Self::Verify(args) => verify::run(args),
            Self::Status => {
                println!("Status command not yet implemented");
                Ok(())
            }
            Self::Version => {
                println!("Version command not yet implemented");
                Ok(())
            }
            Self::Init => {
                println!("Init command not yet implemented");
                Ok(())
            }
        }
    }
}
