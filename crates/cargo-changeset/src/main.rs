mod commands;
mod error;
mod output;
mod verification;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

use crate::commands::Commands;
use crate::error::CliError;

#[derive(Parser)]
#[command(name = "cargo-changeset")]
#[command(bin_name = "cargo-changeset")]
#[command(about = "Manage changesets for Cargo projects", long_about = None)]
struct Cli {
    /// Path to start project discovery from (default: current directory)
    #[arg(long = "path", short = 'C', global = true)]
    path: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let start_path = match resolve_start_path(cli.path) {
        Ok(path) => path,
        Err(e) => {
            print_error(&e);
            return ExitCode::FAILURE;
        }
    };

    let (result, exec_result) = cli.command.execute(&start_path);

    if let Err(e) = result {
        if !exec_result.quiet {
            print_error(&e);
        }
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn resolve_start_path(path: Option<PathBuf>) -> Result<PathBuf, CliError> {
    match path {
        Some(p) => Ok(p),
        None => std::env::current_dir().map_err(CliError::CurrentDir),
    }
}

fn print_error(error: &CliError) {
    eprintln!("error: {error}");

    let mut source = std::error::Error::source(error);
    while let Some(cause) = source {
        eprintln!("caused by: {cause}");
        source = std::error::Error::source(cause);
    }
}
