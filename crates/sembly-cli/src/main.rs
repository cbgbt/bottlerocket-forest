use clap::Parser;
use miette::Result;

mod index;
mod theme;

/// Semantic search and knowledge indexing tool
#[derive(Parser)]
#[command(version, about, styles = clap_cargo::style::CLAP_STYLING)]
struct Args {
    #[command(flatten)]
    command: index::IndexCommand,
}

fn main() -> Result<()> {
    miette::set_panic_hook();
    let args = Args::parse();
    Ok(index::run(args.command)?)
}
