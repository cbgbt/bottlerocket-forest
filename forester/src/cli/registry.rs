use crate::{config, registry};
use argh::FromArgs;
use snafu::{ResultExt, Snafu};

/// Manage local OCI registry
#[derive(FromArgs)]
#[argh(subcommand, name = "registry")]
pub struct RegistryCommand {
    #[argh(subcommand)]
    subcommand: RegistrySubcommand,
}

#[derive(FromArgs)]
#[argh(subcommand)]
enum RegistrySubcommand {
    Start(StartArgs),
    Stop(StopArgs),
    Status(StatusArgs),
    Clean(CleanArgs),
    Logs(LogsArgs),
}

/// Start the local registry
#[derive(FromArgs)]
#[argh(subcommand, name = "start")]
struct StartArgs {}

/// Stop the local registry
#[derive(FromArgs)]
#[argh(subcommand, name = "stop")]
struct StopArgs {}

/// Check registry status
#[derive(FromArgs)]
#[argh(subcommand, name = "status")]
struct StatusArgs {}

/// Remove registry data
#[derive(FromArgs)]
#[argh(subcommand, name = "clean")]
struct CleanArgs {}

/// Show registry logs
#[derive(FromArgs)]
#[argh(subcommand, name = "logs")]
struct LogsArgs {
    /// follow log output
    #[argh(switch, short = 'f')]
    follow: bool,
}

pub fn run(cmd: RegistryCommand) -> Result<(), RegistryError> {
    use registry_error::*;

    let config = config::load_config().context(ConfigSnafu)?;

    match cmd.subcommand {
        RegistrySubcommand::Start(_) => start(&config),
        RegistrySubcommand::Stop(_) => stop(&config),
        RegistrySubcommand::Status(_) => status(&config),
        RegistrySubcommand::Clean(_) => clean(&config),
        RegistrySubcommand::Logs(args) => logs(&config, args.follow),
    }
}

fn start(config: &registry::RegistryConfig) -> Result<(), RegistryError> {
    use registry_error::*;

    let url = registry::start(config).context(OperationSnafu)?;
    println!("Registry started successfully");
    println!("Available at: {}", url);
    Ok(())
}

fn stop(config: &registry::RegistryConfig) -> Result<(), RegistryError> {
    use registry_error::*;

    registry::stop(config).context(OperationSnafu)?;
    println!("Registry stopped");
    Ok(())
}

fn status(config: &registry::RegistryConfig) -> Result<(), RegistryError> {
    use registry_error::*;

    let status = registry::status(config).context(OperationSnafu)?;

    let is_running = matches!(status.state, registry::RegistryState::Running { .. });

    match status.state {
        registry::RegistryState::NotCreated => {
            println!("Registry: Not created");
        }
        registry::RegistryState::Stopped => {
            println!("Registry: Stopped");
        }
        registry::RegistryState::Running { url } => {
            println!("Registry: Running");
            println!("URL: {}", url);
        }
    }

    println!(
        "Volume: {}",
        if status.volume_exists {
            "exists"
        } else {
            "not found"
        }
    );

    if is_running {
        Ok(())
    } else {
        NotRunningSnafu.fail()
    }
}

fn clean(config: &registry::RegistryConfig) -> Result<(), RegistryError> {
    use registry_error::*;

    registry::clean(config).context(OperationSnafu)?;
    println!("Registry cleaned");
    Ok(())
}

fn logs(config: &registry::RegistryConfig, follow: bool) -> Result<(), RegistryError> {
    use registry_error::*;

    registry::logs(config, follow).context(OperationSnafu)
}

#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum RegistryError {
    #[snafu(display("Failed to load configuration"))]
    Config { source: config::ConfigError },

    #[snafu(display("Registry operation failed"))]
    Operation { source: registry::RegistryError },

    #[snafu(display("Registry is not running"))]
    NotRunning,
}
