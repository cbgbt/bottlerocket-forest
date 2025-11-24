//! Command-line interface for forester.
//!
//! This module provides the top-level CLI structure and dispatches to subcommands:
//! * [`index`] - Manages the forest repository index
//! * [`registry`] - Manages the local OCI registry
//!
//! # Adding New Commands
//!
//! When implementing new CLI commands, use the [`theme`] module for consistent
//! colorized output:
//!
//! ```ignore
//! use super::theme;
//!
//! // Success/error indicators
//! println!("{} Operation completed", theme::success("✓"));
//! println!("{} Operation failed", theme::error("✗"));
//!
//! // Values and data
//! println!("Found {} items", theme::value(count));
//! println!("URL: {}", theme::url("http://localhost:5000"));
//!
//! // Labels and secondary info
//! println!("Repository: {}", theme::label("bottlerocket"));
//! println!("  {}", theme::muted("Additional details"));
//! ```
//!
//! The theme module ensures visual consistency with clap-cargo's help styling.
//! See [`theme`] for the complete list of available color functions.

mod index;
mod registry;
mod theme;

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
