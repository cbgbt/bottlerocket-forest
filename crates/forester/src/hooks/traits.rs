//! Hook and plugin traits.

use super::{HookContext, Trigger};
use crate::domain::HookConfig;
use snafu::Snafu;

/// A plugin that creates hooks from configuration.
pub trait Plugin: Send + Sync {
    /// Returns the plugin name.
    fn name(&self) -> &str;
    /// Creates a hook from configuration.
    fn create_hook(&self, config: &HookConfig) -> Result<Box<dyn Hook>, PluginError>;
}

/// A hook that executes at specific triggers.
pub trait Hook: Send + Sync {
    /// Returns the triggers this hook responds to.
    fn triggers(&self) -> &[Trigger];
    /// Executes the hook.
    fn execute(&self, ctx: &HookContext) -> Result<(), HookError>;
}

/// Error from plugin operations.
#[derive(Debug, Snafu)]
pub enum PluginError {
    /// Invalid configuration.
    #[snafu(display("Invalid configuration: {message}"))]
    InvalidConfig {
        /// Error message.
        message: String,
    },
}

/// Error returned by hook execution.
#[derive(Debug, Snafu)]
pub enum HookError {
    /// Hook execution failed.
    #[snafu(display("Hook execution failed: {message}"))]
    Execution {
        /// Error message.
        message: String,
    },
}
