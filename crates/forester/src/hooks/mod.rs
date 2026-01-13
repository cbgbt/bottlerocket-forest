//! Plugin/hook system for forester lifecycle events.

mod phase;
mod context;
mod traits;
mod registry;
pub mod builtin;

pub use phase::HookPhase;
pub use context::HookContext;
pub use traits::{Hook, HookError};
pub use registry::{HookRegistry, RunHooksError};
