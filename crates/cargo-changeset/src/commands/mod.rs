mod add;

use changeset_core::{BumpType, ChangeCategory};
use clap::{Args, Subcommand};

use crate::error::Result;

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Add a new changeset
    Add(AddArgs),
    /// Show status of changesets
    Status,
    /// Bump versions based on changesets
    Version,
    /// Initialize changeset configuration
    Init,
}

#[derive(Args)]
pub(crate) struct AddArgs {
    /// Crate(s) to include in the changeset (skips interactive selection)
    #[arg(long = "crate", value_name = "NAME")]
    pub crates: Vec<String>,

    /// Bump type for all crates (major, minor, patch)
    #[arg(long, short = 'b', value_enum)]
    pub bump: Option<BumpType>,

    /// Per-crate bump type: "crate-name:bump-type"
    #[arg(long = "crate-bump", value_name = "NAME:TYPE")]
    pub crate_bumps: Vec<String>,

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

impl Commands {
    pub(crate) fn execute(self) -> Result<()> {
        match self {
            Self::Add(args) => add::run(args),
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
