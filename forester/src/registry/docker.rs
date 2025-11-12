//! Docker container management with compile-time state guarantees.
//!
//! This module uses the typestate pattern to enforce valid container state transitions
//! at compile time. Invalid operations (like stopping a non-running container) are
//! prevented by the type system.
//!
//! # Valid Operations by State
//!
//! ```text
//! Container<NotCreated>
//!   • discover() → ContainerDiscovered
//!   • create()   → Container<Running>
//!
//! Container<Stopped>
//!   • start()    → Container<Running>
//!   • remove()   → Container<NotCreated>
//!
//! Container<Running>
//!   • stop()     → Container<Stopped>
//!   • url()      → RegistryUrl
//!   • logs()     → Result<()>
//! ```
//!
//! # Example
//!
//! ```ignore
//! use forester::registry::docker::{Container, ContainerDiscovered};
//! use forester::registry::types::{ContainerName, ImageRef, RegistryPort, VolumeName};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a container handle (doesn't touch Docker yet)
//! let name = ContainerName::try_new("test-registry")?;
//! let port = RegistryPort::try_new(5000)?;
//! let volume = VolumeName::try_new("test-data")?;
//! let image = ImageRef::try_new("registry:2")?;
//! let container = Container::new(name, port, volume, image);
//!
//! // Discover current state from Docker
//! match container.discover()? {
//!     ContainerDiscovered::NotCreated(c) => {
//!         // Container doesn't exist, create it
//!         let running = c.create()?;
//!         println!("Registry available at {}", running.url());
//!     }
//!     ContainerDiscovered::Stopped(c) => {
//!         // Container exists but is stopped, start it
//!         let running = c.start()?;
//!         println!("Registry available at {}", running.url());
//!     }
//!     ContainerDiscovered::Running(c) => {
//!         // Already running
//!         println!("Registry available at {}", c.url());
//!     }
//! }
//! # Ok(())
//! # }
//! ```

use crate::registry::types::{ContainerName, ImageRef, RegistryPort, RegistryUrl, VolumeName};
use snafu::{ResultExt, Snafu};
use std::marker::PhantomData;
use std::process::Command;

mod sealed {
    pub trait Sealed {}
}

/// Container state: not yet created in Docker
pub struct NotCreated;

/// Container state: exists but is stopped
pub struct Stopped;

/// Container state: exists and is running
pub struct Running;

impl sealed::Sealed for NotCreated {}
impl sealed::Sealed for Stopped {}
impl sealed::Sealed for Running {}

/// Sealed trait for container states
pub trait ContainerState: sealed::Sealed {}
impl ContainerState for NotCreated {}
impl ContainerState for Stopped {}
impl ContainerState for Running {}

/// Type-safe container handle with compile-time state tracking
pub struct Container<S: ContainerState> {
    name: ContainerName,
    port: RegistryPort,
    volume: VolumeName,
    image: ImageRef,
    _state: PhantomData<S>,
}

impl<S: ContainerState> Container<S> {
    #[allow(dead_code)]
    pub fn name(&self) -> &ContainerName {
        &self.name
    }

    #[allow(dead_code)]
    pub fn port(&self) -> RegistryPort {
        self.port
    }
}

impl Container<NotCreated> {
    /// Creates a new container handle without touching Docker
    pub fn new(
        name: ContainerName,
        port: RegistryPort,
        volume: VolumeName,
        image: ImageRef,
    ) -> Self {
        Self {
            name,
            port,
            volume,
            image,
            _state: PhantomData,
        }
    }

    /// Queries Docker to determine the container's current state
    pub fn discover(self) -> Result<ContainerDiscovered, DockerError> {
        use docker_error::*;

        check_docker_available()?;
        check_docker_running()?;

        if !container_exists(&self.name)? {
            return Ok(ContainerDiscovered::NotCreated(self));
        }

        let output = Command::new("docker")
            .args([
                "inspect",
                "--format",
                "{{.State.Running}}",
                self.name.as_ref(),
            ])
            .output()
            .context(CommandFailedSnafu)?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let is_running = stdout.trim() == "true";

        if is_running {
            Ok(ContainerDiscovered::Running(Container {
                name: self.name,
                port: self.port,
                volume: self.volume,
                image: self.image,
                _state: PhantomData,
            }))
        } else {
            Ok(ContainerDiscovered::Stopped(Container {
                name: self.name,
                port: self.port,
                volume: self.volume,
                image: self.image,
                _state: PhantomData,
            }))
        }
    }

    /// Creates and starts a new container in Docker
    pub fn create(self) -> Result<Container<Running>, DockerError> {
        use docker_error::*;

        check_docker_available()?;
        check_docker_running()?;

        let port_mapping = format!("{}:5000", self.port.into_inner());
        let volume_mapping = format!("{}:/var/lib/registry", self.volume.as_ref());

        let output = Command::new("docker")
            .args([
                "run",
                "-d",
                "--name",
                self.name.as_ref(),
                "-p",
                &port_mapping,
                "-v",
                &volume_mapping,
                "--restart",
                "unless-stopped",
                self.image.as_ref(),
            ])
            .output()
            .context(CommandFailedSnafu)?;

        snafu::ensure!(output.status.success(), ContainerStartFailedSnafu);

        Ok(Container {
            name: self.name,
            port: self.port,
            volume: self.volume,
            image: self.image,
            _state: PhantomData,
        })
    }
}

impl Container<Stopped> {
    /// Starts a stopped container
    pub fn start(self) -> Result<Container<Running>, DockerError> {
        use docker_error::*;

        let output = Command::new("docker")
            .args(["start", self.name.as_ref()])
            .output()
            .context(CommandFailedSnafu)?;

        snafu::ensure!(output.status.success(), ContainerStartFailedSnafu);

        Ok(Container {
            name: self.name,
            port: self.port,
            volume: self.volume,
            image: self.image,
            _state: PhantomData,
        })
    }

    /// Removes a stopped container from Docker
    pub fn remove(self) -> Result<Container<NotCreated>, DockerError> {
        use docker_error::*;

        let output = Command::new("docker")
            .args(["rm", self.name.as_ref()])
            .output()
            .context(CommandFailedSnafu)?;

        snafu::ensure!(output.status.success(), ContainerRemoveFailedSnafu);

        Ok(Container {
            name: self.name,
            port: self.port,
            volume: self.volume,
            image: self.image,
            _state: PhantomData,
        })
    }
}

impl Container<Running> {
    /// Returns the URL where the registry is accessible
    pub fn url(&self) -> RegistryUrl {
        RegistryUrl::builder()
            .host("localhost")
            .port(self.port)
            .build()
    }

    /// Stops a running container
    pub fn stop(self) -> Result<Container<Stopped>, DockerError> {
        use docker_error::*;

        let output = Command::new("docker")
            .args(["stop", self.name.as_ref()])
            .output()
            .context(CommandFailedSnafu)?;

        snafu::ensure!(output.status.success(), ContainerStopFailedSnafu);

        Ok(Container {
            name: self.name,
            port: self.port,
            volume: self.volume,
            image: self.image,
            _state: PhantomData,
        })
    }

    /// Retrieves container logs
    pub fn logs(&self, follow: bool) -> Result<(), DockerError> {
        use docker_error::*;

        let mut cmd = Command::new("docker");
        cmd.args(["logs", self.name.as_ref()]);

        if follow {
            cmd.arg("--follow");
        }

        let status = cmd.status().context(CommandFailedSnafu)?;

        snafu::ensure!(status.success(), LogsFailedSnafu);

        Ok(())
    }
}

/// Result of discovering a container's current state in Docker
pub enum ContainerDiscovered {
    NotCreated(Container<NotCreated>),
    Stopped(Container<Stopped>),
    Running(Container<Running>),
}

/// Checks if a Docker volume exists
pub fn volume_exists(name: &VolumeName) -> Result<bool, DockerError> {
    use docker_error::*;

    let output = Command::new("docker")
        .args([
            "volume",
            "ls",
            "--filter",
            &format!("name=^{}$", name.as_ref()),
            "--format",
            "{{.Name}}",
        ])
        .output()
        .context(CommandFailedSnafu)?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.trim() == name.as_ref())
}

/// Removes a Docker volume
pub fn remove_volume(name: &VolumeName) -> Result<(), DockerError> {
    use docker_error::*;

    let output = Command::new("docker")
        .args(["volume", "rm", name.as_ref()])
        .output()
        .context(CommandFailedSnafu)?;

    snafu::ensure!(output.status.success(), VolumeRemoveFailedSnafu);

    Ok(())
}

fn check_docker_available() -> Result<(), DockerError> {
    use docker_error::*;

    Command::new("docker")
        .arg("--version")
        .output()
        .map_err(|_| DockerNotFoundSnafu.build())?;

    Ok(())
}

fn check_docker_running() -> Result<(), DockerError> {
    use docker_error::*;

    let output = Command::new("docker")
        .arg("info")
        .output()
        .map_err(|_| DockerNotFoundSnafu.build())?;

    snafu::ensure!(output.status.success(), DockerNotRunningSnafu);

    Ok(())
}

fn container_exists(name: &ContainerName) -> Result<bool, DockerError> {
    use docker_error::*;

    let output = Command::new("docker")
        .args([
            "ps",
            "-a",
            "--filter",
            &format!("name=^{}$", name.as_ref()),
            "--format",
            "{{.Names}}",
        ])
        .output()
        .context(CommandFailedSnafu)?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.trim() == name.as_ref())
}

#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum DockerError {
    #[snafu(display("Docker is not installed or not in PATH. Install Docker and ensure it's in your PATH"))]
    DockerNotFound,

    #[snafu(display("Docker daemon is not running. Start Docker with: sudo systemctl start docker"))]
    DockerNotRunning,

    #[snafu(display("Failed to execute docker command"))]
    CommandFailed { source: std::io::Error },

    #[snafu(display("Failed to start container. The port may already be in use"))]
    ContainerStartFailed,

    #[snafu(display("Failed to stop container"))]
    ContainerStopFailed,

    #[snafu(display("Failed to remove container"))]
    ContainerRemoveFailed,

    #[snafu(display("Failed to remove volume. Ensure the volume is not in use"))]
    VolumeRemoveFailed,

    #[snafu(display("Failed to get container logs"))]
    LogsFailed,
}
