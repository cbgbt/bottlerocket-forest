use crate::registry::types::RegistryConfig;
use snafu::{ResultExt, Snafu};

/// Load registry configuration from environment variables
///
/// Reads configuration from environment variables prefixed with `FORESTER_REGISTRY_`.
/// Falls back to defaults if variables are not set.
pub fn load_config() -> Result<RegistryConfig, ConfigError> {
    use config_error::*;

    dotenvy::dotenv().ok();

    let config: RegistryConfig = envy::prefixed("FORESTER_REGISTRY_")
        .from_env()
        .context(LoadFailedSnafu)?;

    let port = config.port;
    Ok(config.with_port(port))
}

#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum ConfigError {
    #[snafu(display("Failed to load configuration from environment"))]
    LoadFailed { source: envy::Error },
}
