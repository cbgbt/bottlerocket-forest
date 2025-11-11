use crate::registry::types::{ContainerName, RegistryPort, RegistryState, RegistryUrl};
use serde::Deserialize;
use snafu::{ResultExt, Snafu};
use std::process::Command;

pub fn check_docker_available() -> Result<(), DockerError> {
    use docker_error::*;

    Command::new("docker")
        .arg("--version")
        .output()
        .map_err(|_| DockerNotFoundSnafu.build())?;

    Ok(())
}

pub fn check_docker_running() -> Result<(), DockerError> {
    use docker_error::*;

    let output = Command::new("docker")
        .arg("info")
        .output()
        .map_err(|_| DockerNotFoundSnafu.build())?;

    snafu::ensure!(output.status.success(), DockerNotRunningSnafu);

    Ok(())
}

pub fn container_exists(name: &ContainerName) -> Result<bool, DockerError> {
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

pub fn get_container_state(
    name: &ContainerName,
    port: RegistryPort,
) -> Result<RegistryState, DockerError> {
    use docker_error::*;

    if !container_exists(name)? {
        return Ok(RegistryState::NotCreated);
    }

    let output = Command::new("docker")
        .args(["inspect", "--format", "{{.State.Running}}", name.as_ref()])
        .output()
        .context(CommandFailedSnafu)?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let is_running = stdout.trim() == "true";

    if is_running {
        let url = RegistryUrl::builder().host("localhost").port(port).build();
        Ok(RegistryState::Running { url })
    } else {
        Ok(RegistryState::Stopped)
    }
}

pub fn start_container(name: &ContainerName) -> Result<(), DockerError> {
    use docker_error::*;

    let output = Command::new("docker")
        .args(["start", name.as_ref()])
        .output()
        .context(CommandFailedSnafu)?;

    snafu::ensure!(output.status.success(), ContainerStartFailedSnafu);

    Ok(())
}

pub fn create_and_run_container(
    name: &ContainerName,
    port: RegistryPort,
    volume: &crate::registry::types::VolumeName,
    image: &crate::registry::types::ImageRef,
) -> Result<(), DockerError> {
    use docker_error::*;

    let port_mapping = format!("{}:5000", port.into_inner());
    let volume_mapping = format!("{}:/var/lib/registry", volume.as_ref());

    let output = Command::new("docker")
        .args([
            "run",
            "-d",
            "--name",
            name.as_ref(),
            "-p",
            &port_mapping,
            "-v",
            &volume_mapping,
            "--restart",
            "unless-stopped",
            image.as_ref(),
        ])
        .output()
        .context(CommandFailedSnafu)?;

    snafu::ensure!(output.status.success(), ContainerStartFailedSnafu);

    Ok(())
}

pub fn stop_container(name: &ContainerName) -> Result<(), DockerError> {
    use docker_error::*;

    let output = Command::new("docker")
        .args(["stop", name.as_ref()])
        .output()
        .context(CommandFailedSnafu)?;

    snafu::ensure!(output.status.success(), ContainerStopFailedSnafu);

    Ok(())
}

pub fn remove_container(name: &ContainerName) -> Result<(), DockerError> {
    use docker_error::*;

    let output = Command::new("docker")
        .args(["rm", name.as_ref()])
        .output()
        .context(CommandFailedSnafu)?;

    snafu::ensure!(output.status.success(), ContainerRemoveFailedSnafu);

    Ok(())
}

pub fn remove_volume(name: &crate::registry::types::VolumeName) -> Result<(), DockerError> {
    use docker_error::*;

    let output = Command::new("docker")
        .args(["volume", "rm", name.as_ref()])
        .output()
        .context(CommandFailedSnafu)?;

    snafu::ensure!(output.status.success(), VolumeRemoveFailedSnafu);

    Ok(())
}

pub fn volume_exists(name: &crate::registry::types::VolumeName) -> Result<bool, DockerError> {
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

pub fn get_container_logs(name: &ContainerName, follow: bool) -> Result<(), DockerError> {
    use docker_error::*;

    let mut cmd = Command::new("docker");
    cmd.args(["logs", name.as_ref()]);

    if follow {
        cmd.arg("--follow");
    }

    let status = cmd.status().context(CommandFailedSnafu)?;

    snafu::ensure!(status.success(), LogsFailedSnafu);

    Ok(())
}

#[derive(Debug, Deserialize)]
struct ContainerInfo {
    #[serde(rename = "Names")]
    names: String,
}

#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum DockerError {
    #[snafu(display("Docker is not installed or not in PATH"))]
    DockerNotFound,

    #[snafu(display("Docker daemon is not running"))]
    DockerNotRunning,

    #[snafu(display("Failed to execute docker command"))]
    CommandFailed { source: std::io::Error },

    #[snafu(display("Failed to parse docker output"))]
    ParseFailed,

    #[snafu(display("Failed to start container"))]
    ContainerStartFailed,

    #[snafu(display("Failed to stop container"))]
    ContainerStopFailed,

    #[snafu(display("Failed to remove container"))]
    ContainerRemoveFailed,

    #[snafu(display("Failed to remove volume"))]
    VolumeRemoveFailed,

    #[snafu(display("Failed to get container logs"))]
    LogsFailed,
}
