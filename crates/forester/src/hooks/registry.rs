//! Hook registry for managing and executing hooks.

use super::{Hook, HookContext, HookPhase};
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
