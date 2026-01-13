//! Grove removal operation.

use std::path::{Path, PathBuf};

use snafu::{ResultExt, Snafu};

use crate::domain::{ForestConfig, ForestRoot, GroveName};
use crate::events::{EventEmitter, ForesterEvent};
use crate::git::BareRepository;
use crate::hooks::{HookContext, HookRegistry, Trigger};

/// Removes a grove and its worktrees.
pub struct GroveRemoveOperation<'a> {
    forest_root: &'a ForestRoot,
    config: &'a ForestConfig,
    hooks: &'a HookRegistry,
    emitter: &'a dyn EventEmitter,
    verbose: bool,
}

impl<'a> GroveRemoveOperation<'a> {
    /// Creates a new grove removal operation.
    pub fn new(
        forest_root: &'a ForestRoot,
        config: &'a ForestConfig,
        hooks: &'a HookRegistry,
        emitter: &'a dyn EventEmitter,
        verbose: bool,
    ) -> Self {
        Self {
            forest_root,
            config,
            hooks,
            emitter,
            verbose,
        }
    }

    /// Executes the grove removal.
    pub fn execute(&self, name: &GroveName, force: bool) -> Result<(), GroveRemoveError> {
        use grove_remove_error::*;

        let grove_path = self.forest_root.groves_dir().join(name.to_string());

        if !grove_path.exists() {
            return Err(GroveRemoveError::NotFound {
                name: name.to_string(),
            });
        }

        self.emitter.emit(&ForesterEvent::GroveRemoving {
            name: name.to_string(),
        });

        for member in &self.config.forest.member {
            let bare_path = self
                .forest_root
                .bare_dir()
                .join(format!("{}.git", member.name));
            let member_path = grove_path.join(&member.path);
            let bare = BareRepository::new(&bare_path, &member.name);
            let _ = bare.remove_worktree(&member_path, force);
        }

        std::fs::remove_dir_all(&grove_path).context(RemoveDirSnafu { path: &grove_path })?;

        let ctx = self.hook_context(name, &grove_path);
        self.hooks
            .run_hooks(Trigger::PostGroveRemove, &ctx)
            .context(HookSnafu)?;

        self.emitter.emit(&ForesterEvent::GroveRemoved {
            name: name.to_string(),
        });
        Ok(())
    }

    fn hook_context(&self, name: &GroveName, path: &Path) -> HookContext {
        HookContext::builder()
            .forest_root(self.forest_root.path())
            .trigger(Trigger::PostGroveRemove)
            .grove_name(name.to_string())
            .grove_path(path.to_path_buf())
            .verbose(self.verbose)
            .build()
    }
}

/// Errors from removing a grove.
#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum GroveRemoveError {
    /// Grove not found.
    #[snafu(display("Grove '{name}' not found"))]
    NotFound {
        /// Grove name.
        name: String,
    },

    /// Failed to remove a directory.
    #[snafu(display("Failed to remove directory"))]
    RemoveDir {
        /// Path that could not be removed.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },

    /// A hook failed during execution.
    #[snafu(display("Hook failed"))]
    Hook {
        /// Underlying hook error.
        source: crate::hooks::RunHooksError,
    },
}
