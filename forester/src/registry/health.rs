use crate::registry::types::RegistryUrl;
use snafu::{ResultExt, Snafu};
use std::time::Duration;

const HEALTH_CHECK_TIMEOUT_SECS: u64 = 2;
const INITIAL_BACKOFF_MILLIS: u64 = 100;
const MAX_BACKOFF_SECS: u64 = 1;

/// Checks if the registry is responding to health checks
pub fn check_ready(url: &RegistryUrl) -> Result<(), HealthError> {
    use health_error::*;

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(HEALTH_CHECK_TIMEOUT_SECS))
        .build()
        .context(ClientBuildSnafu)?;

    let response = client
        .get(format!("{}/v2/", url))
        .send()
        .context(ConnectionFailedSnafu)?;

    snafu::ensure!(
        response.status().is_success(),
        UnexpectedStatusSnafu {
            status: response.status().as_u16()
        }
    );

    Ok(())
}

/// Waits for the registry to become ready with exponential backoff
pub fn wait_until_ready(url: &RegistryUrl, timeout: Duration) -> Result<(), HealthError> {
    use health_error::*;

    let start = std::time::Instant::now();
    let mut delay = Duration::from_millis(INITIAL_BACKOFF_MILLIS);

    loop {
        if start.elapsed() > timeout {
            return TimeoutSnafu.fail();
        }

        if check_ready(url).is_ok() {
            return Ok(());
        }

        std::thread::sleep(delay);
        delay = (delay * 2).min(Duration::from_secs(MAX_BACKOFF_SECS));
    }
}

#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum HealthError {
    #[snafu(display("Health check timed out"))]
    Timeout,

    #[snafu(display("Failed to connect to registry"))]
    ConnectionFailed { source: reqwest::Error },

    #[snafu(display("Unexpected HTTP status: {status}"))]
    UnexpectedStatus { status: u16 },

    #[snafu(display("Failed to build HTTP client"))]
    ClientBuild { source: reqwest::Error },
}
