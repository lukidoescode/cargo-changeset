mod add;

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
