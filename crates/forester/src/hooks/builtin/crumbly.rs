//! Crumbly indexing plugin.

use crate::domain::HookConfig;
use crate::hooks::{Hook, HookContext, HookError, Plugin, PluginError, Trigger};
use std::process::{Command, Stdio};

/// Plugin for crumbly indexing operations.
pub struct CrumblyPlugin;

impl Plugin for CrumblyPlugin {
    fn name(&self) -> &str {
        "crumbly"
    }

    fn create_hook(&self, config: &HookConfig) -> Result<Box<dyn Hook>, PluginError> {
        let command = config
            .command
            .clone()
            .ok_or_else(|| PluginError::InvalidConfig {
                message: "crumbly hook requires 'command' field".to_string(),
            })?;
        let triggers: Vec<Trigger> = config
            .triggers
            .iter()
            .filter_map(|s| Trigger::parse(s))
            .collect();
        Ok(Box::new(CrumblyHook { command, triggers }))
    }
}

struct CrumblyHook {
    command: String,
    triggers: Vec<Trigger>,
}

impl CrumblyHook {
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
    fn triggers(&self) -> &[Trigger] {
        &self.triggers
    }

    fn execute(&self, ctx: &HookContext) -> Result<(), HookError> {
        if !Self::is_installed() {
            return Ok(());
        }

        let mut cmd = Command::new("crumbly");
        for (k, v) in ctx.env_vars() {
            cmd.env(k, v);
        }

        match self.command.as_str() {
            "cache-bare" => {
                let bare_dir = ctx.forest_root.join(".forest/bare");
                cmd.args(["cache", "bare-git"])
                    .arg(&bare_dir)
                    .current_dir(&ctx.forest_root);
            }
            "update-context" => {
                let context_arg = ctx
                    .grove_path
                    .as_ref()
                    .and_then(|p| p.strip_prefix(&ctx.forest_root).ok())
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| ".".to_string());
                cmd.args(["update", "--context", &context_arg])
                    .current_dir(&ctx.forest_root);
            }
            other => {
                return Err(HookError::Execution {
                    message: format!("Unknown crumbly command: {}", other),
                });
            }
        }

        if !ctx.verbose {
            cmd.stdout(Stdio::null()).stderr(Stdio::null());
        }

        let status = cmd.status().map_err(|e| HookError::Execution {
            message: format!("Failed to run crumbly: {}", e),
        })?;

        if !status.success() {
            return Err(HookError::Execution {
                message: format!("crumbly {} failed", self.command),
            });
        }

        Ok(())
    }
}
