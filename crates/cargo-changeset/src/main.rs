mod commands;
mod environment;
mod error;
mod interaction;
mod output;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

use crate::commands::Commands;
use crate::error::CliError;

#[derive(Parser)]
#[command(name = "cargo")]
#[command(bin_name = "cargo")]
enum CargoCli {
    Changeset(ChangesetCli),
}

#[derive(Parser)]
#[command(name = "cargo-changeset")]
#[command(version = env!("CARGO_CHANGESET_VERSION"))]
#[command(about = "Manage changesets for Cargo projects", long_about = None)]
struct ChangesetCli {
    #[arg(long = "path", short = 'C', global = true)]
    path: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

fn main() -> ExitCode {
    let cli = match CargoCli::try_parse() {
        Ok(CargoCli::Changeset(cli)) => cli,
        Err(e) if !e.use_stderr() => e.exit(),
        Err(_) => ChangesetCli::parse(),
    };

    let start_path = match resolve_start_path(cli.path) {
        Ok(path) => path,
        Err(e) => {
            output::error_format::print_error(&e);
            return ExitCode::FAILURE;
        }
    };

    let (result, exec_result) = cli.command.execute(&start_path);

    if let Err(e) = result {
        if !exec_result.quiet {
            output::error_format::print_error(&e);
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
