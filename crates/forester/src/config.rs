use crate::registry::types::{RegistryConfig, RegistryRuntimeConfig};
use snafu::{ResultExt, Snafu};

/// Load registry configuration from environment variables
///
/// Reads configuration from environment variables prefixed with `FORESTER_REGISTRY_`.
/// Falls back to defaults if variables are not set.
pub fn load_config() -> Result<RegistryRuntimeConfig, ConfigError> {
    use config_error::*;

    dotenvy::dotenv().ok();

    let config: RegistryConfig = envy::prefixed("FORESTER_REGISTRY_")
        .from_env()
        .context(LoadFailedSnafu)?;

    Ok(config.into_runtime())
}

#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum ConfigError {
    #[snafu(display("Failed to load configuration from environment"))]
    LoadFailed { source: envy::Error },
}
