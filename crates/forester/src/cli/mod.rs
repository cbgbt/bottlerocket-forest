//! CLI for forester.

mod args;
mod commands;

use args::{Cli, ColorChoice, Command};
use clap::Parser;
use std::io::IsTerminal;

/// Parses CLI arguments and executes the requested command.
pub fn run() -> miette::Result<()> {
    let cli = Cli::parse();

    match cli.color {
        ColorChoice::Always => owo_colors::set_override(true),
        ColorChoice::Never => owo_colors::set_override(false),
        ColorChoice::Auto => {
            if !std::io::stdout().is_terminal() {
                owo_colors::set_override(false);
            }
        }
    }

    match cli.command {
        Command::Init(args) => commands::init(args),
        Command::Seed(args) => commands::seed(args),
        Command::Grove(cmd) => commands::grove(cmd),
    }
}
