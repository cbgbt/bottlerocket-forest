use crate::{config, registry};
use chrono::Utc;
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
    /// List images in the registry
    List,
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
        RegistrySubcommand::List => list(&config),
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

/// List all images in the registry
fn list(config: &registry::RegistryConfig) -> Result<(), RegistryError> {
    use registry_error::*;

    let status = registry::status(config).context(OperationSnafu)?;

    let url = match status.state {
        registry::RegistryState::Running { url } => url,
        registry::RegistryState::Stopped => {
            return Err(RegistryError::RegistryNotRunning {
                message: "Registry is stopped. Start it with 'forester registry start'".to_string(),
            });
        }
        registry::RegistryState::NotCreated => {
            return Err(RegistryError::RegistryNotRunning {
                message: "Registry not created. Start it with 'forester registry start'"
                    .to_string(),
            });
        }
    };

    let images = registry::list_images(&url).context(CatalogSnafu)?;

    if images.is_empty() {
        println!("{}", "No images found in registry".dimmed());
        return Ok(());
    }

    // Group images by repository
    let mut repos: std::collections::BTreeMap<_, Vec<_>> = std::collections::BTreeMap::new();
    for image in images {
        repos
            .entry(image.repository.clone())
            .or_default()
            .push(image);
    }

    println!(
        "{} {} with {} {}:\n",
        "Registry".cyan().bold(),
        url.to_string().dimmed(),
        repos.len().to_string().cyan(),
        if repos.len() == 1 {
            "repository"
        } else {
            "repositories"
        }
    );

    let repo_count = repos.len();
    for (idx, (repo, images)) in repos.iter().enumerate() {
        let is_last_repo = idx == repo_count - 1;
        let repo_prefix = if is_last_repo {
            "└──"
        } else {
            "├──"
        };
        let tag_prefix = if is_last_repo { "    " } else { "│   " };

        println!("{} {}", repo_prefix.dimmed(), repo.as_ref().bright_white());

        let image_count = images.len();
        for (img_idx, image) in images.iter().enumerate() {
            let is_last_image = img_idx == image_count - 1;
            let img_symbol = if is_last_image {
                "└──"
            } else {
                "├──"
            };

            let size_mb = image.size_bytes as f64 / 1_000_000.0;
            let digest_short = &image.digest.chars().take(19).collect::<String>();
            
            let time_ago = image.created.map(|created| {
                let duration = Utc::now().signed_duration_since(created);
                if duration.num_days() > 365 {
                    format!("{}y ago", duration.num_days() / 365)
                } else if duration.num_days() > 30 {
                    format!("{}mo ago", duration.num_days() / 30)
                } else if duration.num_days() > 0 {
                    format!("{}d ago", duration.num_days())
                } else if duration.num_hours() > 0 {
                    format!("{}h ago", duration.num_hours())
                } else if duration.num_minutes() > 0 {
                    format!("{}m ago", duration.num_minutes())
                } else {
                    "just now".to_string()
                }
            });

            println!(
                "{}{} {} {} {} {}",
                tag_prefix.dimmed(),
                img_symbol.dimmed(),
                format!(":{}", image.tag.as_ref()).blue(),
                format!("{:.1}MB", size_mb).yellow(),
                time_ago.as_deref().unwrap_or("").green(),
                digest_short.dimmed()
            );
        }
    }

    Ok(())
}

#[derive(Debug, Snafu, miette::Diagnostic)]
#[snafu(module)]
pub enum RegistryError {
    #[snafu(display("Failed to load configuration"))]
    #[diagnostic(
        code(forester::registry::config_failed),
        help("Check that FORESTER_REGISTRY_XXX environment variables are correct")
    )]
    Config { source: config::ConfigError },

    #[snafu(display("Registry operation failed"))]
    #[diagnostic(
        code(forester::registry::operation_failed),
        help("Ensure Docker is installed and running: sudo systemctl start docker")
    )]
    Operation { source: registry::RegistryError },

    #[snafu(display("Failed to list registry images"))]
    #[diagnostic(
        code(forester::registry::catalog_failed),
        help("Ensure the registry is running with 'forester registry start'")
    )]
    Catalog { source: registry::CatalogError },

    #[snafu(display("Registry is not running"))]
    #[diagnostic(code(forester::registry::not_running), help("{message}"))]
    RegistryNotRunning { message: String },
}
