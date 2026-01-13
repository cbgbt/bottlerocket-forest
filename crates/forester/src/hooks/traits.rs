//! Hook trait and error types.

use super::{HookContext, HookPhase};
use snafu::Snafu;

/// A hook that executes at specific lifecycle phases.
pub trait Hook: Send + Sync {
    /// Returns the hook's name.
    fn name(&self) -> &str;
    /// Returns the phases this hook runs in.
    fn phases(&self) -> &[HookPhase];
    /// Executes the hook.
    fn execute(&self, ctx: &HookContext) -> Result<(), HookError>;
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
