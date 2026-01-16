//! Script execution plugin.

use crate::domain::HookConfig;
use crate::hooks::{Hook, HookContext, HookError, Plugin, PluginError, Trigger};
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Plugin for executing arbitrary scripts.
pub struct ExecPlugin;

impl Plugin for ExecPlugin {
    fn name(&self) -> &str {
        "exec"
    }

    fn create_hook(&self, config: &HookConfig) -> Result<Box<dyn Hook>, PluginError> {
        let path = config
            .path
            .clone()
            .ok_or_else(|| PluginError::InvalidConfig {
                message: "exec hook requires 'path' field".to_string(),
            })?;
        let args = config.args.clone().unwrap_or_default();
        let triggers: Vec<Trigger> = config
            .triggers
            .iter()
            .filter_map(|s| Trigger::parse(s))
            .collect();
        Ok(Box::new(ExecHook {
            path,
            args,
            triggers,
        }))
    }
}

struct ExecHook {
    path: PathBuf,
    args: Vec<String>,
    triggers: Vec<Trigger>,
}

impl Hook for ExecHook {
    fn triggers(&self) -> &[Trigger] {
        &self.triggers
    }

    fn execute(&self, ctx: &HookContext) -> Result<(), HookError> {
        let script_path = if self.path.is_relative() {
            ctx.forest_root.path().join(&self.path)
        } else {
            self.path.clone()
        };

        let mut cmd = Command::new(&script_path);
        cmd.args(&self.args).current_dir(ctx.forest_root.path());

        for (k, v) in ctx.env_vars() {
            cmd.env(k, v);
        }

        if !ctx.verbose {
            cmd.stdout(Stdio::null()).stderr(Stdio::null());
        }

        let status = cmd.status().map_err(|e| HookError::Execution {
            message: format!("Failed to run {}: {}", script_path.display(), e),
        })?;

        if !status.success() {
            return Err(HookError::Execution {
                message: format!("Script {} failed", script_path.display()),
            });
        }

        Ok(())
    }
}
