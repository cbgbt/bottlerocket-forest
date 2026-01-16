//! Grove removal operation.

use std::path::{Path, PathBuf};

use snafu::{ResultExt, Snafu};

use crate::domain::{ForestRoot, GroveName};
use crate::events::{EventEmitter, ForesterEvent};
use crate::hooks::{HookContext, HookRegistry, Trigger};

/// Removes a grove and its cloned repositories.
pub struct GroveRemoveOperation<'a> {
    forest_root: &'a ForestRoot,
    hooks: &'a HookRegistry,
    emitter: &'a dyn EventEmitter,
    verbose: bool,
}

impl<'a> GroveRemoveOperation<'a> {
    /// Creates a new grove removal operation.
    pub fn new(
        forest_root: &'a ForestRoot,
        hooks: &'a HookRegistry,
        emitter: &'a dyn EventEmitter,
        verbose: bool,
    ) -> Self {
        Self {
            forest_root,
            hooks,
            emitter,
            verbose,
        }
    }

    /// Executes the grove removal.
    pub fn execute(&self, name: &GroveName, _force: bool) -> Result<(), GroveRemoveError> {
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
            .forest_root(self.forest_root.clone())
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
