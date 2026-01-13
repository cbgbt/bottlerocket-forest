//! Built-in hooks.

mod crumbly;

use crate::hooks::{Hook, HookPhase};
pub use crumbly::CrumblyHook;

/// Metadata for a builtin hook.
pub struct BuiltinHookMeta {
    /// Hook name.
    pub name: &'static str,
    /// Human-readable description.
    pub description: &'static str,
    /// Whether enabled by default.
    pub default_enabled: bool,
    /// Default phases this hook runs in.
    pub default_phases: &'static [HookPhase],
}

/// Returns metadata for all builtin hooks.
pub fn all_builtin_metas() -> Vec<BuiltinHookMeta> {
    vec![BuiltinHookMeta {
        name: "crumbly",
        description: "Runs crumbly indexing after seed and grove creation",
        default_enabled: true,
        default_phases: &[HookPhase::PostSeed, HookPhase::PostGroveCreate],
    }]
}

/// Creates a hook instance by name.
pub fn create_hook(name: &str) -> Option<Box<dyn Hook>> {
    match name {
        "crumbly" => Some(Box::new(CrumblyHook::new())),
        _ => None,
    }
}
