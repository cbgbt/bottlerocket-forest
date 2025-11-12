mod docker;
mod health;
pub mod types;

use docker::{Container, ContainerDiscovered};
use snafu::{ResultExt, Snafu};
use std::time::Duration;
pub use types::{RegistryConfig, RegistryState, RegistryStatus, RegistryUrl};

const REGISTRY_STARTUP_TIMEOUT_SECS: u64 = 10;

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
