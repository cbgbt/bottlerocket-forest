mod docker;
mod health;
pub mod types;

use snafu::Snafu;
pub use types::{RegistryConfig, RegistryState, RegistryStatus, RegistryUrl};

pub fn start(_config: &RegistryConfig) -> Result<RegistryUrl, RegistryError> {
    todo!("Implement registry start")
}

pub fn stop(_config: &RegistryConfig) -> Result<(), RegistryError> {
    todo!("Implement registry stop")
}

pub fn status(_config: &RegistryConfig) -> Result<RegistryStatus, RegistryError> {
    todo!("Implement registry status")
}

pub fn clean(_config: &RegistryConfig) -> Result<(), RegistryError> {
    todo!("Implement registry clean")
}

pub fn logs(_config: &RegistryConfig, _follow: bool) -> Result<(), RegistryError> {
    todo!("Implement registry logs")
}

#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum RegistryError {
    #[snafu(display("Docker is not installed or not in PATH"))]
    DockerNotFound,

    #[snafu(display("Docker daemon is not running"))]
    DockerNotRunning,

    #[snafu(display("Port {port} is already in use"))]
    PortInUse { port: u16 },

    #[snafu(display("Failed to start container"))]
    ContainerStartFailed,

    #[snafu(display("Health check timed out"))]
    HealthCheckTimeout,

    #[snafu(display("Permission denied accessing Docker"))]
    PermissionDenied,

    #[snafu(display("Failed to remove volume"))]
    VolumeRemovalFailed,
}
