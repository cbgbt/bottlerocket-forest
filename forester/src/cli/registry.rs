use crate::{config, registry};
use clap::{Parser, Subcommand};
use owo_colors::OwoColorize;
use snafu::{ResultExt, Snafu};

/// Manage local OCI registry
#[derive(Parser)]
pub struct RegistryCommand {
    #[command(subcommand)]
    subcommand: RegistrySubcommand,
}

#[derive(Subcommand)]
enum RegistrySubcommand {
    /// Start the local registry
    Start,
    /// Stop the local registry
    Stop,
    /// Check registry status
    Status,
    /// Remove registry data
    Clean,
    /// Show registry logs
    Logs(LogsArgs),
}

/// Show registry logs
#[derive(Parser)]
struct LogsArgs {
    /// follow log output
    #[arg(short = 'f', long)]
    follow: bool,
}

pub fn run(cmd: RegistryCommand) -> Result<(), RegistryError> {
    use registry_error::*;

    let config = config::load_config().context(ConfigSnafu)?;

    match cmd.subcommand {
        RegistrySubcommand::Start => start(&config),
        RegistrySubcommand::Stop => stop(&config),
        RegistrySubcommand::Status => status(&config),
        RegistrySubcommand::Clean => clean(&config),
        RegistrySubcommand::Logs(args) => logs(&config, args.follow),
    }
}

/// Start the registry and display the URL
fn start(config: &registry::RegistryConfig) -> Result<(), RegistryError> {
    use registry_error::*;

    let url = registry::start(config).context(OperationSnafu)?;
    println!("{} Registry started successfully", "✓".green().bold());
    println!("  Available at: {}", url.to_string().cyan().underline());
    Ok(())
}

/// Stop the registry and display confirmation
fn stop(config: &registry::RegistryConfig) -> Result<(), RegistryError> {
    use registry_error::*;

    registry::stop(config).context(OperationSnafu)?;
    println!("{} Registry stopped", "✓".green().bold());
    Ok(())
}

/// Display the registry status
fn status(config: &registry::RegistryConfig) -> Result<(), RegistryError> {
    use registry_error::*;

    let status = registry::status(config).context(OperationSnafu)?;

    match status.state {
        registry::RegistryState::NotCreated => {
            println!("Registry: {}", "Not created".dimmed());
        }
        registry::RegistryState::Stopped => {
            println!("Registry: {}", "Stopped".yellow());
        }
        registry::RegistryState::Running { url } => {
            println!("Registry: {} 🚀", "Running".green().bold());
            println!("  URL: {}", url.to_string().cyan().underline());
        }
    }

    println!(
        "Volume: {}",
        if status.volume_exists {
            "exists".green().to_string()
        } else {
            "not found".red().to_string()
        }
    );

    Ok(())
}

/// Clean the registry container and volume
fn clean(config: &registry::RegistryConfig) -> Result<(), RegistryError> {
    use registry_error::*;

    registry::clean(config).context(OperationSnafu)?;
    println!("{} Registry cleaned", "✓".green().bold());
    Ok(())
}

/// Display registry logs
fn logs(config: &registry::RegistryConfig, follow: bool) -> Result<(), RegistryError> {
    use registry_error::*;

    registry::logs(config, follow).context(OperationSnafu)
}

#[derive(Debug, Snafu, miette::Diagnostic)]
#[snafu(module)]
pub enum RegistryError {
    #[snafu(display("Failed to load configuration"))]
    #[diagnostic(
        code(forester::registry::config_failed),
        help("Check that the configuration file is valid and accessible")
    )]
    Config { source: config::ConfigError },

    #[snafu(display("Registry operation failed"))]
    #[diagnostic(
        code(forester::registry::operation_failed),
        help("Check the error details above for specific guidance")
    )]
    Operation { source: registry::RegistryError },
}
