//! Plugin/hook system for forester lifecycle events.

pub mod builtin;
mod context;
mod registry;
mod traits;
mod trigger;

pub use context::HookContext;
pub use registry::{HookRegistry, RegistryError, RunHooksError};
pub use traits::{Hook, HookError, Plugin, PluginError};
pub use trigger::Trigger;
