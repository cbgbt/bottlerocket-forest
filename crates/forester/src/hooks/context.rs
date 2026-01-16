//! Hook execution context.

use super::Trigger;
use crate::domain::ForestRoot;
use bon::Builder;
use std::path::PathBuf;

/// Context passed to hooks during execution.
#[derive(Debug, Clone, Builder)]
#[non_exhaustive]
pub struct HookContext {
    /// Root directory of the forest.
    pub forest_root: ForestRoot,
    /// Current trigger.
    pub trigger: Trigger,
    /// Name of the grove being operated on.
    #[builder(into)]
    pub grove_name: Option<String>,
    /// Path to the grove being operated on.
    #[builder(into)]
    pub grove_path: Option<PathBuf>,
    /// Whether verbose output is enabled.
    #[builder(default)]
    pub verbose: bool,
}

impl HookContext {
    /// Returns environment variables for hook execution.
    pub fn env_vars(&self) -> Vec<(&'static str, String)> {
        let mut vars = vec![
            ("FOREST_ROOT", self.forest_root.path().display().to_string()),
            ("TRIGGER", self.trigger.to_string()),
        ];
        if let Some(name) = &self.grove_name {
            vars.push(("GROVE_NAME", name.clone()));
        }
        if let Some(path) = &self.grove_path {
            vars.push(("GROVE_PATH", path.display().to_string()));
        }
        vars
    }
}
