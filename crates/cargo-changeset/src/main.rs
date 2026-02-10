mod commands;
mod error;

use std::process::ExitCode;

use clap::Parser;

use crate::commands::Commands;
use crate::error::CliError;

#[derive(Parser)]
#[command(name = "cargo-changeset")]
#[command(bin_name = "cargo-changeset")]
#[command(about = "Manage changesets for Cargo projects", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

fn main() -> ExitCode {
    if let Err(e) = run() {
        print_error(&e);
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run() -> error::Result<()> {
    let cli = Cli::parse();
    cli.command.execute()
}

fn print_error(error: &CliError) {
    eprintln!("error: {error}");

    let mut source = std::error::Error::source(error);
    while let Some(cause) = source {
        eprintln!("caused by: {cause}");
        source = std::error::Error::source(cause);
    }
}
