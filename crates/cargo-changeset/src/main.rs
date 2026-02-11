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
    let cli = Cli::parse();
    let (result, exec_result) = cli.command.execute();

    if let Err(e) = result {
        if !exec_result.quiet {
            print_error(&e);
        }
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn print_error(error: &CliError) {
    eprintln!("error: {error}");

    let mut source = std::error::Error::source(error);
    while let Some(cause) = source {
        eprintln!("caused by: {cause}");
        source = std::error::Error::source(cause);
    }
}
