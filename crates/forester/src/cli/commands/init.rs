//! Init command handler.

use crate::cli::args::InitArgs;
use crate::events::ConsoleEmitter;
use crate::grove::GroveContext;
use crate::ops::InitOperation;
use miette::Diagnostic;
use owo_colors::OwoColorize;
use snafu::Snafu;

#[derive(Debug, Snafu, Diagnostic)]
#[snafu(module)]
pub enum InitError {
    #[snafu(display("cannot initialize forest inside grove '{name}'"))]
    #[diagnostic(help("Change to a directory outside any grove before running init"))]
    InsideGrove { name: String },
}

pub fn run(args: InitArgs) -> miette::Result<()> {
    use init_error::*;

    if let Ok(Some(ctx)) = GroveContext::detect() {
        return Err(InsideGroveSnafu {
            name: ctx.name().to_string(),
        }
        .build()
        .into());
    }

    let cwd = std::env::current_dir()
        .map_err(|e| miette::miette!("Failed to get current directory: {}", e))?;
    let emitter = ConsoleEmitter::new(false);
    let op = InitOperation::new(cwd, &emitter);
    op.execute().map_err(|e| miette::miette!("{}", e))?;

    if let Some(name) = args.name {
        println!("\nForest '{}' initialized. Next steps:", name.cyan());
    } else {
        println!("\nForest initialized. Next steps:");
    }
    println!(
        "  1. Edit {} to add member repositories",
        "forester.toml".cyan()
    );
    println!("  2. Run {} to clone and set up", "forester seed".cyan());

    Ok(())
}
