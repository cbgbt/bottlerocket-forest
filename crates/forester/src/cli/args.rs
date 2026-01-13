//! CLI argument definitions.

use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Controls terminal color output behavior.
#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum ColorChoice {
    #[default]
    Auto,
    Always,
    Never,
}

#[derive(Parser)]
#[command(name = "forester")]
#[command(about = "Generic forest management for multi-repo projects")]
#[command(version)]
pub struct Cli {
    #[arg(long, global = true, default_value = "auto")]
    pub color: ColorChoice,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Initialize a new forest in the current directory
    Init(InitArgs),
    /// Clone all member repositories and set up the forest
    Seed(SeedArgs),
    /// Manage forest groves
    #[command(subcommand)]
    Grove(GroveCommand),
}

#[derive(Args, Debug)]
pub struct InitArgs {
    /// Forest name
    #[arg(short, long)]
    pub name: Option<String>,
}

#[derive(Args, Debug)]
pub struct SeedArgs {
    /// Path to forester.toml (default: current directory)
    #[arg(short, long)]
    pub config: Option<PathBuf>,
    /// Show verbose output
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Subcommand, Debug)]
pub enum GroveCommand {
    /// Create a new grove
    Create(GroveCreateArgs),
    /// List all groves
    List,
    /// Remove a grove
    Remove(GroveRemoveArgs),
    /// Show current grove status
    Status,
    /// Print current grove name
    Current,
}

#[derive(Args, Debug)]
pub struct GroveCreateArgs {
    pub name: String,
    #[arg(short, long)]
    pub branch: Option<String>,
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Args, Debug)]
pub struct GroveRemoveArgs {
    pub name: String,
    #[arg(short, long)]
    pub force: bool,
}
