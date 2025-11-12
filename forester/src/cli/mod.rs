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
    Registry(registry::RegistryCommand),
}

pub fn run() -> Result<(), CliError> {
    use cli_error::*;

    let args: Args = argh::from_env();

    match args.command {
        Command::Registry(cmd) => registry::run(cmd).context(RegistrySnafu)?,
    }

    Ok(())
}

#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum CliError {
    #[snafu(display("Registry command failed"))]
    Registry { source: registry::RegistryError },
}
