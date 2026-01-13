//! Grove creation operation.

use std::path::{Path, PathBuf};

use snafu::{ResultExt, Snafu};

use crate::domain::{ForestConfig, ForestRoot, GroveName, GroveRoot};
use crate::events::{EventEmitter, ForesterEvent};
use crate::git::BareRepository;
use crate::hooks::{HookContext, HookRegistry, Trigger};

/// Creates a grove with worktrees for all members.
pub struct GroveCreateOperation<'a> {
    forest_root: &'a ForestRoot,
    config: &'a ForestConfig,
    hooks: &'a HookRegistry,
    emitter: &'a dyn EventEmitter,
    verbose: bool,
}

impl<'a> GroveCreateOperation<'a> {
    /// Creates a new grove creation operation.
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

    /// Executes the grove creation.
    pub fn execute(
        &self,
        name: &GroveName,
        branch: Option<&str>,
    ) -> Result<GroveRoot, GroveCreateError> {
        use grove_create_error::*;

        let grove_path = self.forest_root.groves_dir().join(name.to_string());
        let grove_root = GroveRoot::builder().path(&grove_path).build();

        self.emitter.emit(&ForesterEvent::GroveCreating {
            name: name.to_string(),
        });

        if grove_path.exists() {
            return Err(GroveCreateError::AlreadyExists {
                name: name.to_string(),
            });
        }

        std::fs::create_dir_all(&grove_path).context(CreateDirSnafu { path: &grove_path })?;
        std::fs::create_dir_all(grove_root.marker_dir()).context(CreateDirSnafu {
            path: grove_root.marker_dir(),
        })?;

        for member in &self.config.forest.member {
            self.create_member_worktree(member, &grove_path, name, branch)?;
        }

        self.create_symlinks(&grove_path, self.forest_root.path())?;

        let ctx = self.hook_context(name, &grove_path);
        self.hooks
            .run_hooks(Trigger::PostGroveCreate, &ctx)
            .context(HookSnafu)?;

        self.emitter.emit(&ForesterEvent::GroveCreated {
            name: name.to_string(),
        });
        Ok(grove_root)
    }

    fn create_member_worktree(
        &self,
        member: &crate::domain::Member,
        grove_path: &Path,
        grove_name: &GroveName,
        branch: Option<&str>,
    ) -> Result<(), GroveCreateError> {
        use grove_create_error::*;

        let bare_path = self
            .forest_root
            .bare_dir()
            .join(format!("{}.git", member.name));
        let member_path = grove_path.join(&member.path);

        if let Some(parent) = member_path.parent() {
            std::fs::create_dir_all(parent).context(CreateDirSnafu {
                path: parent.to_path_buf(),
            })?;
        }

        let bare = BareRepository::new(&bare_path, &member.name);
        let target_branch = branch.unwrap_or_else(|| member.branch());
        let use_new_branch = grove_name.to_string() != "develop" && branch.is_none();

        if use_new_branch {
            let new_branch = format!("{}/{}", grove_name, member.name);
            bare.create_worktree_new_branch(
                &member_path,
                &new_branch,
                member.branch(),
                !self.verbose,
            )
            .context(WorktreeSnafu {
                member: &member.name,
            })?;
        } else {
            bare.create_worktree(&member_path, target_branch, !self.verbose)
                .context(WorktreeSnafu {
                    member: &member.name,
                })?;
        }

        self.emitter.emit(&ForesterEvent::GroveWorktreeCreated {
            member: member.name.clone(),
            path: member_path,
        });

        Ok(())
    }

    fn create_symlinks(
        &self,
        grove_path: &Path,
        forest_path: &Path,
    ) -> Result<(), GroveCreateError> {
        use grove_create_error::*;

        let Some(grove_config) = &self.config.grove else {
            return Ok(());
        };

        for entry in &grove_config.symlink {
            let src = forest_path.join(&entry.source);
            let tgt = grove_path.join(&entry.target);

            if let Some(parent) = tgt.parent() {
                std::fs::create_dir_all(parent).context(CreateDirSnafu {
                    path: parent.to_path_buf(),
                })?;
            }

            std::os::unix::fs::symlink(&src, &tgt).context(SymlinkSnafu {
                src: src.clone(),
                tgt: tgt.clone(),
            })?;
        }

        Ok(())
    }

    fn hook_context(&self, name: &GroveName, path: &Path) -> HookContext {
        HookContext::builder()
            .forest_root(self.forest_root.path())
            .trigger(Trigger::PostGroveCreate)
            .grove_name(name.to_string())
            .grove_path(path.to_path_buf())
            .verbose(self.verbose)
            .build()
    }
}

/// Errors from creating a grove.
#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum GroveCreateError {
    /// Grove already exists.
    #[snafu(display("Grove '{name}' already exists"))]
    AlreadyExists {
        /// Grove name.
        name: String,
    },

    /// Failed to create a directory.
    #[snafu(display("Failed to create directory"))]
    CreateDir {
        /// Path that could not be created.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },

    /// Failed to create a worktree.
    #[snafu(display("Failed to create worktree for '{member}'"))]
    Worktree {
        /// Member name.
        member: String,
        /// Underlying worktree error.
        source: crate::git::CreateWorktreeError,
    },

    /// Failed to create a symlink.
    #[snafu(display("Failed to create symlink"))]
    Symlink {
        /// Source path.
        src: PathBuf,
        /// Target path.
        tgt: PathBuf,
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
