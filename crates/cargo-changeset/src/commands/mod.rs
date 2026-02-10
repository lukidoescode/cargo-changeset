mod add;

use clap::Subcommand;

use crate::error::Result;

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Add a new changeset
    Add,
    /// Show status of changesets
    Status,
    /// Bump versions based on changesets
    Version,
    /// Initialize changeset configuration
    Init,
}

impl Commands {
    pub(crate) fn execute(self) -> Result<()> {
        match self {
            Self::Add => add::run(),
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
