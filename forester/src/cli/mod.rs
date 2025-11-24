//! Command-line interface for forester.
//!
//! This module provides the top-level CLI structure and dispatches to subcommands:
//! * [`index`] - Manages the forest repository index
//! * [`registry`] - Manages the local OCI registry

mod index;
mod registry;

use clap::{Parser, Subcommand};
use snafu::{ResultExt, Snafu};

/// Bottlerocket development orchestration tool
#[derive(Parser)]
#[command(version, about, styles = clap_cargo::style::CLAP_STYLING)]
pub struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Index(index::IndexCommand),
    Registry(registry::RegistryCommand),
}

pub fn run() -> Result<(), CliError> {
    use cli_error::*;

    let args = Args::parse();

    match args.command {
        Command::Index(cmd) => index::run(cmd).context(IndexSnafu)?,
        Command::Registry(cmd) => registry::run(cmd).context(RegistrySnafu)?,
    }

    Ok(())
}

#[derive(Debug, Snafu, miette::Diagnostic)]
#[snafu(module)]
pub enum CliError {
    #[snafu(display("Index command failed"))]
    #[diagnostic(
        code(forester::cli::index_command_failed),
        help("Check the error details above for specific guidance")
    )]
    Index { source: index::IndexError },

    #[snafu(display("Registry command failed"))]
    #[diagnostic(
        code(forester::cli::registry_command_failed),
        help("Check the error details above for specific guidance")
    )]
    Registry { source: registry::RegistryError },
}
