//! Hook registry for managing and executing hooks.

use super::builtin::{all_builtin_metas, create_hook};
use super::{Hook, HookContext, HookPhase};
use crate::domain::HookConfig;
use snafu::Snafu;

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

    /// Creates a registry from config, instantiating enabled builtin hooks.
    pub fn from_config(hook_configs: &[HookConfig]) -> Self {
        let mut registry = Self::new();
        let metas = all_builtin_metas();

        for meta in &metas {
            let cfg = hook_configs.iter().find(|c| c.name == meta.name);
            let enabled = cfg
                .and_then(|c| c.enabled)
                .unwrap_or(meta.default_enabled);

            if enabled {
                if let Some(hook) = create_hook(meta.name) {
                    registry.hooks.push(hook);
                }
            }
        }

        registry
    }

    /// Registers a hook.
    pub fn register(&mut self, hook: impl Hook + 'static) {
        self.hooks.push(Box::new(hook));
    }

    /// Runs all hooks registered for the given phase.
    pub fn run_hooks(&self, phase: HookPhase, ctx: &HookContext) -> Result<(), RunHooksError> {
        use run_hooks_error::*;
        for hook in self.hooks.iter().filter(|h| h.phases().contains(&phase)) {
            hook.execute(ctx).map_err(|e| {
                HookFailedSnafu {
                    name: hook.name().to_string(),
                    message: e.to_string(),
                }
                .build()
            })?;
        }
        Ok(())
    }
}

/// Error from running hooks.
#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum RunHooksError {
    /// A hook failed during execution.
    #[snafu(display("Hook '{name}' failed: {message}"))]
    HookFailed {
        /// Name of the failed hook.
        name: String,
        /// Error message.
        message: String,
    },
}
