mod catalog;
mod docker;
mod health;
pub mod types;

pub use catalog::{CatalogError, ImageTag, RegistryImage, RepositoryName, list_images};
use docker::{Container, ContainerDiscovered};
use snafu::{ResultExt, Snafu};
use std::time::Duration;
pub use types::{RegistryConfig, RegistryState, RegistryStatus, RegistryUrl};

const REGISTRY_STARTUP_TIMEOUT_SECS: u64 = 10;

/// Start the local OCI registry container
///
/// Creates and starts a Docker container running the registry image. If the container
/// already exists but is stopped, it will be started. Waits for the registry to become
/// healthy before returning.
pub fn start(config: &RegistryConfig) -> Result<RegistryUrl, RegistryError> {
    use registry_error::*;

    let container = Container::new(
        config.container_name.clone(),
        config.port,
        config.volume_name.clone(),
        config.image.clone(),
    );

    let running = match container.discover().context(DockerSnafu)? {
        ContainerDiscovered::Running(c) => c,
        ContainerDiscovered::Stopped(c) => c.start().context(DockerSnafu)?,
        ContainerDiscovered::NotCreated(c) => c.create().context(DockerSnafu)?,
    };

    let url = running.url();

    health::wait_until_ready(&url, Duration::from_secs(REGISTRY_STARTUP_TIMEOUT_SECS))
        .context(HealthCheckSnafu)?;

    Ok(url)
}

/// Stop the local OCI registry container
///
/// Stops the registry container if it is running. Does nothing if the container
/// is already stopped or does not exist.
pub fn stop(config: &RegistryConfig) -> Result<(), RegistryError> {
    use registry_error::*;

    let container = Container::new(
        config.container_name.clone(),
        config.port,
        config.volume_name.clone(),
        config.image.clone(),
    );

    match container.discover().context(DockerSnafu)? {
        ContainerDiscovered::Running(c) => {
            c.stop().context(DockerSnafu)?;
        }
        ContainerDiscovered::Stopped(_) | ContainerDiscovered::NotCreated(_) => {}
    }

    Ok(())
}

/// Get the status of the local OCI registry
///
/// Returns information about the registry container state (running, stopped, or not created)
/// and whether the data volume exists.
pub fn status(config: &RegistryConfig) -> Result<RegistryStatus, RegistryError> {
    use registry_error::*;

    let container = Container::new(
        config.container_name.clone(),
        config.port,
        config.volume_name.clone(),
        config.image.clone(),
    );

    let state = match container.discover().context(DockerSnafu)? {
        ContainerDiscovered::NotCreated(_) => RegistryState::NotCreated,
        ContainerDiscovered::Stopped(_) => RegistryState::Stopped,
        ContainerDiscovered::Running(c) => RegistryState::Running { url: c.url() },
    };

    let volume_exists = docker::volume_exists(&config.volume_name).context(DockerSnafu)?;

    Ok(RegistryStatus {
        state,
        volume_exists,
    })
}

/// Remove the registry container and data volume
///
/// Stops and removes the registry container if it exists, then removes the data volume.
/// This permanently deletes all registry data.
pub fn clean(config: &RegistryConfig) -> Result<(), RegistryError> {
    use registry_error::*;

    let container = Container::new(
        config.container_name.clone(),
        config.port,
        config.volume_name.clone(),
        config.image.clone(),
    );

    match container.discover().context(DockerSnafu)? {
        ContainerDiscovered::Running(c) => {
            let stopped = c.stop().context(DockerSnafu)?;
            stopped.remove().context(DockerSnafu)?;
        }
        ContainerDiscovered::Stopped(c) => {
            c.remove().context(DockerSnafu)?;
        }
        ContainerDiscovered::NotCreated(_) => {}
    }

    if docker::volume_exists(&config.volume_name).context(DockerSnafu)? {
        docker::remove_volume(&config.volume_name).context(DockerSnafu)?;
    }

    Ok(())
}

/// Display logs from the registry container
///
/// Shows the container logs. If `follow` is true, streams logs continuously until interrupted.
/// Returns an error if the container is not running.
pub fn logs(config: &RegistryConfig, follow: bool) -> Result<(), RegistryError> {
    use registry_error::*;

    let container = Container::new(
        config.container_name.clone(),
        config.port,
        config.volume_name.clone(),
        config.image.clone(),
    );

    match container.discover().context(DockerSnafu)? {
        ContainerDiscovered::Running(c) => {
            c.logs(follow).context(DockerSnafu)?;
        }
        ContainerDiscovered::Stopped(_) | ContainerDiscovered::NotCreated(_) => {
            return ContainerNotRunningSnafu.fail();
        }
    }

    Ok(())
}

#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum RegistryError {
    #[snafu(display("Docker operation failed"))]
    Docker { source: docker::DockerError },

    #[snafu(display("Health check failed"))]
    HealthCheck { source: health::HealthError },

    #[snafu(display("Container is not running"))]
    ContainerNotRunning,
}
