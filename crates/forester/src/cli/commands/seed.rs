//! Seed command handler.

use crate::cli::args::SeedArgs;
use crate::domain::{ForestConfig, ForestRoot};
use crate::events::ConsoleEmitter;
use crate::grove::GroveContext;
use crate::hooks::HookRegistry;
use crate::hooks::builtin::CrumblyHook;
use crate::ops::SeedOperation;
use owo_colors::OwoColorize;
use std::path::Path;

pub fn run(args: SeedArgs) -> miette::Result<()> {
    if let Ok(Some(ctx)) = GroveContext::detect() {
        println!(
            "{} Running seed from grove '{}'. This affects the entire forest.",
            "!".yellow(),
            ctx.name().cyan()
        );
    }

    let (forest_path, config) = if let Some(config_path) = args.config {
        let cfg = load_config(&config_path)?;
        let root = config_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        (root, cfg)
    } else {
        find_config()?
    };

    let forest_root = ForestRoot::builder().path(&forest_path).build();
    let emitter = ConsoleEmitter::new(args.verbose);
    let mut hooks = HookRegistry::new();
    hooks.register(CrumblyHook::new());

    let op = SeedOperation::new(&forest_root, &config, &hooks, &emitter, args.verbose);
    op.execute().map_err(|e| miette::miette!("{}", e))?;

    println!();
    println!("To start working, create a grove:");
    println!("  {}", "forester grove create <name>".cyan());

    Ok(())
}

fn load_config(path: &Path) -> miette::Result<ForestConfig> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| miette::miette!("Failed to read {}: {}", path.display(), e))?;
    toml::from_str(&content)
        .map_err(|e| miette::miette!("Failed to parse {}: {}", path.display(), e))
}

fn find_config() -> miette::Result<(std::path::PathBuf, ForestConfig)> {
    let cwd = std::env::current_dir()
        .map_err(|e| miette::miette!("Failed to get current directory: {}", e))?;
    let mut dir = cwd;
    loop {
        let config_path = dir.join("forester.toml");
        if config_path.exists() {
            let config = load_config(&config_path)?;
            return Ok((dir, config));
        }
        if !dir.pop() {
            return Err(miette::miette!("No forester.toml found"));
        }
    }
}
