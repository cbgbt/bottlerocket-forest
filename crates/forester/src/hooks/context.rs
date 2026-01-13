//! Hook execution context.

use bon::Builder;
use std::path::PathBuf;
use super::HookPhase;

/// Context passed to hooks during execution.
#[derive(Debug, Clone, Builder)]
#[non_exhaustive]
pub struct HookContext {
    /// Root directory of the forest.
    #[builder(into)]
    pub forest_root: PathBuf,
    /// Current lifecycle phase.
    pub phase: HookPhase,
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
