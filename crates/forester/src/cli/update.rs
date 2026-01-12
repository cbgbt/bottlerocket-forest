//! Update command - fetch latest changes for all member repos.

use crate::forest::ForestConfig;
use crate::grove::ForestManager;
use clap::Args;
use owo_colors::OwoColorize;
use std::path::PathBuf;
use tracing::instrument;

#[derive(Args, Debug)]
pub struct UpdateArgs {
    /// Path to forester.toml (default: current directory)
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Show verbose output
    #[arg(short, long)]
    verbose: bool,
}

#[instrument(err)]
pub fn run(args: UpdateArgs) -> miette::Result<()> {
    let (forest_root, config) = if let Some(config_path) = args.config {
        let config = ForestConfig::load(&config_path)?;
        let root = config_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .to_path_buf();
        (root, config)
    } else {
        ForestConfig::find()?
    };

    if args.verbose {
        println!(
            "Updating forest '{}' at {}",
            config.forest.name.cyan(),
            forest_root.display()
        );
    }

    let manager = ForestManager::new(forest_root, config);
    manager.update(args.verbose)?;

    println!("{}", "✓ Forest updated successfully".green());

    Ok(())
}
