mod index;
mod registry;

use argh::FromArgs;
use snafu::{ResultExt, Snafu};

/// Bottlerocket development orchestration tool
#[derive(FromArgs)]
pub struct Args {
    #[argh(subcommand)]
    command: Command,
}

#[derive(FromArgs)]
#[argh(subcommand)]
enum Command {
    Index(index::IndexCommand),
    Registry(registry::RegistryCommand),
}

pub fn run() -> Result<(), CliError> {
    use cli_error::*;

    let args: Args = argh::from_env();

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
