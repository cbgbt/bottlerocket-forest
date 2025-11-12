use crate::registry::types::RegistryConfig;
use snafu::{ResultExt, Snafu};

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
