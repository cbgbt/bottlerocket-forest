use std::io::IsTerminal;

use clap::{Parser, Subcommand, ValueEnum};
use miette::Result;

mod context;
mod gc;
mod index;
mod theme;

/// Semantic search and knowledge indexing tool.
#[derive(Parser)]
#[command(version, about, styles = clap_cargo::style::CLAP_STYLING, args_conflicts_with_subcommands = false)]
struct Args {
    /// When to use colors (auto, always, never)
    #[arg(long, global = true, default_value = "auto")]
    color: ColorChoice,

    #[command(subcommand)]
    command: Command,
}

/// When to use colored output.
#[derive(Clone, Copy, Debug, Default, ValueEnum)]
enum ColorChoice {
    /// Use colors only when output is a terminal
    #[default]
    Auto,
    /// Always use colors
    Always,
    /// Never use colors
    Never,
}

/// Top-level commands.
#[derive(Subcommand)]
enum Command {
    /// Build the knowledge index.
    Build(index::BuildArgs),
    /// Rebuild the knowledge index from scratch.
    Rebuild(index::RebuildArgs),
    /// Update the knowledge index incrementally.
    Update(index::UpdateArgs),
    /// Clear all chunks from the index.
    Clear(index::ClearArgs),
    /// Search the knowledge index.
    Search(index::SearchArgs),
    /// Show index status and statistics.
    Status(index::StatusArgs),
    /// Manage contexts.
    Context(context::ContextCommand),
    /// Remove orphaned chunks not referenced by any context.
    Gc(gc::GcArgs),
}

fn main() -> Result<()> {
    miette::set_panic_hook();
    let args = Args::parse();

    match args.color {
        ColorChoice::Always => owo_colors::set_override(true),
        ColorChoice::Never => owo_colors::set_override(false),
        ColorChoice::Auto => {
            if !std::io::stdout().is_terminal() {
                owo_colors::set_override(false);
            }
        }
    }

    match args.command {
        Command::Build(args) => Ok(index::handle_build(args)?),
        Command::Rebuild(args) => Ok(index::handle_rebuild(args)?),
        Command::Update(args) => Ok(index::handle_update(args)?),
        Command::Clear(args) => Ok(index::handle_clear(args)?),
        Command::Search(args) => Ok(index::handle_search(args)?),
        Command::Status(args) => Ok(index::handle_status(args)?),
        Command::Context(cmd) => Ok(context::run(cmd)?),
        Command::Gc(args) => Ok(gc::handle_gc(args)?),
    }
}
