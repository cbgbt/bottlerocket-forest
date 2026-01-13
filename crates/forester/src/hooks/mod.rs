//! Plugin/hook system for forester lifecycle events.

pub mod builtin;
mod context;
mod phase;
mod registry;
mod traits;

pub use context::HookContext;
pub use phase::HookPhase;
pub use registry::{HookRegistry, RunHooksError};
pub use traits::{Hook, HookError};
