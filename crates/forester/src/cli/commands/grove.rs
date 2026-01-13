//! Grove command handlers.

use crate::cli::args::{GroveCommand, GroveCreateArgs, GroveRemoveArgs};
use crate::domain::{ForestConfig, ForestRoot, GroveName};
use crate::events::ConsoleEmitter;
use crate::grove::GroveContext;
use crate::hooks::HookRegistry;
use crate::ops::{GroveCreateOperation, GroveListOperation, GroveRemoveOperation};
use miette::Diagnostic;
use owo_colors::OwoColorize;
use snafu::Snafu;
use std::path::Path;

#[derive(Debug, Snafu, Diagnostic)]
#[snafu(module)]
pub enum GroveError {
    #[snafu(display("cannot remove grove '{name}' while inside it"))]
    #[diagnostic(help("Change to a directory outside the grove before removing it"))]
    RemoveCurrentGrove { name: String },
}

pub fn run(cmd: GroveCommand) -> miette::Result<()> {
    match cmd {
        GroveCommand::Create(args) => create(args),
        GroveCommand::List => list(),
        GroveCommand::Remove(args) => remove(args),
        GroveCommand::Status => status_cmd(),
        GroveCommand::Current => current(),
    }
}

fn create(args: GroveCreateArgs) -> miette::Result<()> {
    let (forest_path, config) = find_config()?;
    let forest_root = ForestRoot::builder().path(&forest_path).build();
    let emitter = ConsoleEmitter::new(args.verbose);
    let hooks = HookRegistry::from_config(&config.hook);

    let grove_name = GroveName::try_new(args.name).map_err(|e| miette::miette!("{}", e))?;

    let op = GroveCreateOperation::new(&forest_root, &config, &hooks, &emitter, args.verbose);
    op.execute(&grove_name, args.branch.as_deref())
        .map_err(|e| miette::miette!("{}", e))?;

    Ok(())
}

fn list() -> miette::Result<()> {
    let (forest_path, _config) = find_config()?;
    let forest_root = ForestRoot::builder().path(&forest_path).build();
    let emitter = ConsoleEmitter::new(false);
    let cwd = std::env::current_dir().ok();

    let op = GroveListOperation::new(&forest_root, &emitter, cwd);
    let groves = op.execute().map_err(|e| miette::miette!("{}", e))?;

    if groves.is_empty() {
        println!("No groves found");
    } else {
        println!("Forest groves:");
        for g in groves {
            if g.is_current {
                println!("* {} {}", g.name.to_string().cyan(), "(current)".dimmed());
            } else {
                println!("  {}", g.name.to_string().cyan());
            }
        }
    }

    Ok(())
}

fn remove(args: GroveRemoveArgs) -> miette::Result<()> {
    use grove_error::*;

    if let Ok(Some(ctx)) = GroveContext::detect()
        && ctx.name() == args.name
    {
        return Err(RemoveCurrentGroveSnafu { name: args.name }.build().into());
    }

    let (forest_path, config) = find_config()?;
    let forest_root = ForestRoot::builder().path(&forest_path).build();
    let emitter = ConsoleEmitter::new(false);
    let hooks = HookRegistry::from_config(&config.hook);

    let grove_name = GroveName::try_new(args.name).map_err(|e| miette::miette!("{}", e))?;

    let op = GroveRemoveOperation::new(&forest_root, &config, &hooks, &emitter, false);
    op.execute(&grove_name, args.force)
        .map_err(|e| miette::miette!("{}", e))?;

    Ok(())
}

fn status_cmd() -> miette::Result<()> {
    let (forest_path, _) = find_config()?;

    if let Some(ctx) = GroveContext::detect().ok().flatten() {
        println!("Grove:       {}", ctx.name().cyan());
        println!("Grove root:  {}", ctx.grove_root().display());
        println!("Forest root: {}", ctx.forest_root().display());
    } else {
        println!("Not in a grove");
        println!("Forest root: {}", forest_path.display());
    }

    Ok(())
}

fn current() -> miette::Result<()> {
    if let Some(ctx) = GroveContext::detect().ok().flatten() {
        println!("{}", ctx.name());
    } else {
        std::process::exit(1);
    }
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
