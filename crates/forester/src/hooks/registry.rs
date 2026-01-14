//! Hook registry for managing and executing hooks.

use snafu::Snafu;

use super::builtin::create_hook;
use super::{Hook, HookContext, PluginError, Trigger};
use crate::domain::HookConfig;

/// Registry for managing hooks.
#[derive(Default)]
pub struct HookRegistry {
    hooks: Vec<Box<dyn Hook>>,
}

impl HookRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a registry from configuration.
    pub fn from_config(configs: &[HookConfig]) -> Result<Self, RegistryError> {
        let mut registry = Self::new();
        for config in configs {
            let hook = create_hook(config).map_err(|e| RegistryError::CreateHook {
                name: config.name.clone(),
                source: e,
            })?;
            registry.hooks.push(hook);
        }
        Ok(registry)
    }

    /// Runs all hooks for the given trigger.
    pub fn run_hooks(&self, trigger: Trigger, ctx: &HookContext) -> Result<(), RunHooksError> {
        for hook in self
            .hooks
            .iter()
            .filter(|h| h.triggers().contains(&trigger))
        {
            hook.execute(ctx).map_err(|e| RunHooksError::HookFailed {
                message: e.to_string(),
            })?;
        }
        Ok(())
    }
}

/// Error from creating a registry.
#[derive(Debug, Snafu)]
pub enum RegistryError {
    /// Failed to create a hook.
    #[snafu(display("Failed to create hook '{name}': {source}"))]
    CreateHook {
        /// Hook name.
        name: String,
        /// Underlying error.
        source: PluginError,
    },
}

/// Error from running hooks.
#[derive(Debug, Snafu)]
pub enum RunHooksError {
    /// A hook failed.
    #[snafu(display("Hook failed: {message}"))]
    HookFailed {
        /// Error message.
        message: String,
    },
}
