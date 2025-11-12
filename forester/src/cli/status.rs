use argh::FromArgs;
use snafu::Snafu;

/// Show forest development status
#[derive(FromArgs)]
#[argh(subcommand, name = "status")]
pub struct StatusCommand {}

pub fn run(_cmd: StatusCommand) -> Result<(), StatusError> {
    println!("Forest Status:");
    println!("  Registry: Not implemented");
    println!("  Built kits: Not implemented");
    println!("  Built variants: Not implemented");
    Ok(())
}

#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum StatusError {}
