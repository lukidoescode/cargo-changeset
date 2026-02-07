use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "cargo-changeset")]
#[command(bin_name = "cargo-changeset")]
#[command(about = "Manage changesets for Cargo projects", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Add a new changeset
    Add,
    /// Show status of changesets
    Status,
    /// Bump versions based on changesets
    Version,
    /// Initialize changeset configuration
    Init,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Add => {
            println!("Add command not yet implemented");
        }
        Commands::Status => {
            println!("Status command not yet implemented");
        }
        Commands::Version => {
            println!("Version command not yet implemented");
        }
        Commands::Init => {
            println!("Init command not yet implemented");
        }
    }

    Ok(())
}
