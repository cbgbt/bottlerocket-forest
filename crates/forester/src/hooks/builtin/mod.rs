//! Built-in plugins.

mod crumbly;
mod exec;

use crate::domain::HookConfig;
use crate::hooks::{Hook, Plugin, PluginError};
pub use crumbly::CrumblyPlugin;
pub use exec::ExecPlugin;

static PLUGINS: &[&dyn Plugin] = &[&CrumblyPlugin, &ExecPlugin];

/// Creates a hook from configuration using the appropriate plugin.
pub fn create_hook(config: &HookConfig) -> Result<Box<dyn Hook>, PluginError> {
    for plugin in PLUGINS {
        if plugin.name() == config.name {
            return plugin.create_hook(config);
        }
    }
    Err(PluginError::InvalidConfig {
        message: format!("Unknown plugin: {}", config.name),
    })
}
