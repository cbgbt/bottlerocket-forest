//! Crumbly indexing hook.

use crate::hooks::{Hook, HookContext, HookError, HookPhase};
use std::process::{Command, Stdio};

/// Hook that runs crumbly indexing after seed and grove creation.
#[derive(Debug, Default)]
pub struct CrumblyHook;

impl CrumblyHook {
    /// Creates a new crumbly hook.
    pub fn new() -> Self {
        Self
    }

    fn is_installed() -> bool {
        Command::new("which")
            .arg("crumbly")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

impl Hook for CrumblyHook {
    fn name(&self) -> &str {
        "crumbly"
    }

    fn phases(&self) -> &[HookPhase] {
        &[HookPhase::PostSeed, HookPhase::PostGroveCreate]
    }

    fn execute(&self, ctx: &HookContext) -> Result<(), HookError> {
        if !Self::is_installed() {
            return Ok(());
        }

        let crumbly_dir = ctx.forest_root.join(".crumbly");
        let subcommand = if crumbly_dir.exists() { "update" } else { "build" };

        let context_arg = ctx.grove_path
            .as_ref()
            .and_then(|p| p.strip_prefix(&ctx.forest_root).ok())
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| ".".to_string());

        let mut cmd = Command::new("crumbly");
        cmd.args([subcommand, "--context", &context_arg])
            .current_dir(&ctx.forest_root);
        if !ctx.verbose {
            cmd.stdout(Stdio::null()).stderr(Stdio::null());
        }

        let status = cmd.status().map_err(|e| HookError::Execution {
            message: format!("Failed to run crumbly: {}", e),
        })?;

        if !status.success() {
            return Err(HookError::Execution {
                message: format!("crumbly {} failed", subcommand),
            });
        }

        Ok(())
    }
}
