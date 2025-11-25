//! Local OCI registry management commands.
//!
//! Provides subcommands for controlling a Docker-based OCI registry container:
//! * Starting and stopping the registry
//! * Checking registry status and viewing logs
//! * Listing stored images
//! * Cleaning registry data

use crate::{config, registry};
use chrono::Utc;
use clap::{Parser, Subcommand};
use snafu::{ResultExt, Snafu};
use timeago::Formatter;

use super::theme;

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
    /// Follow log output
    #[arg(short = 'f', long)]
    follow: bool,
}

/// Executes the registry subcommand
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

/// Starts the local OCI registry and prints the URL
fn start(config: &registry::RegistryRuntimeConfig) -> Result<(), RegistryError> {
    use registry_error::*;

    let url = registry::start(config).context(OperationSnafu)?;
    println!("{} Registry started successfully", theme::success("✓"));
    println!("  Available at: {}", theme::url(&url));
    Ok(())
}

/// Stops the local OCI registry
fn stop(config: &registry::RegistryRuntimeConfig) -> Result<(), RegistryError> {
    use registry_error::*;

    registry::stop(config).context(OperationSnafu)?;
    println!("{} Registry stopped", theme::success("✓"));
    Ok(())
}

/// Queries and displays the current registry state
fn status(config: &registry::RegistryRuntimeConfig) -> Result<(), RegistryError> {
    use registry_error::*;

    let status = registry::status(config).context(OperationSnafu)?;

    match status.state {
        registry::RegistryState::NotCreated => {
            println!("Registry: {}", theme::muted("Not created"));
        }
        registry::RegistryState::Stopped => {
            println!("Registry: {}", theme::warning("Stopped"));
        }
        registry::RegistryState::Running { ref url } => {
            println!("Registry: {} 🚀", theme::success("Running"));
            println!("  URL: {}", theme::url(url));

            // Fetch image stats if registry is running
            if let Ok(images) = registry::list_images(url) {
                let total_logical_size: u64 = images.iter().map(|img| img.size_bytes).sum();
                let size_gb = total_logical_size as f64 / 1_000_000_000.0;

                println!(
                    "  Images: {} ({:.2} GB logical)",
                    theme::value(images.len()),
                    size_gb
                );
                println!(
                    "  {}",
                    theme::muted_italic(
                        "Note: Actual disk usage may be lower due to layer deduplication"
                    )
                );
            }
        }
    }

    println!(
        "Volume: {}",
        if status.volume_exists {
            theme::success("exists")
        } else {
            theme::error("not found")
        }
    );

    Ok(())
}

/// Removes the registry container and associated volume
fn clean(config: &registry::RegistryRuntimeConfig) -> Result<(), RegistryError> {
    use registry_error::*;

    registry::clean(config).context(OperationSnafu)?;
    println!("{} Registry cleaned", theme::success("✓"));
    Ok(())
}

/// Streams logs from the registry container
fn logs(config: &registry::RegistryRuntimeConfig, follow: bool) -> Result<(), RegistryError> {
    use registry_error::*;

    registry::logs(config, follow).context(OperationSnafu)
}

/// Queries and displays all images stored in the registry
fn list(config: &registry::RegistryRuntimeConfig) -> Result<(), RegistryError> {
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
        println!("{}", theme::muted("No images found in registry"));
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
        theme::value("Registry"),
        theme::muted(&url),
        theme::value(repos.len()),
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

        println!(
            "{} {}",
            theme::muted(repo_prefix),
            theme::label(repo.as_ref())
        );

        // Calculate max tag width for alignment
        let max_tag_width = images
            .iter()
            .map(|img| img.tag.as_ref().len())
            .max()
            .unwrap_or(0);

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
                let formatter = Formatter::new();
                format!("Pushed {}", formatter.convert_chrono(created, Utc::now()))
            });

            let tag_display = format!(":{}", image.tag.as_ref());
            let size_display = format!("{:.1}MB", size_mb);
            let time_display = time_ago.as_deref().unwrap_or("");

            // Calculate padding manually to avoid color code interference
            let tag_width = max_tag_width + 1;
            let tag_padding = tag_width.saturating_sub(tag_display.len());
            let size_padding = 8usize.saturating_sub(size_display.len());
            let time_padding = 22usize.saturating_sub(time_display.len());

            print!("{}", theme::muted(tag_prefix));
            print!("{} ", theme::muted(img_symbol));
            print!("{}{} ", theme::tag(&tag_display), " ".repeat(tag_padding));
            print!(
                "{}{} ",
                " ".repeat(size_padding),
                theme::size(&size_display)
            );
            print!("{}{} ", " ".repeat(time_padding), theme::time(time_display));
            println!("{}", theme::muted(digest_short));
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
