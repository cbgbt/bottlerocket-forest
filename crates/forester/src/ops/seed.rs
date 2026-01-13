//! Forest seeding operation.

use std::path::PathBuf;

use snafu::{ResultExt, Snafu};

use crate::domain::{ForestConfig, ForestRoot};
use crate::events::{EventEmitter, ForesterEvent};
use crate::git::BareRepository;
use crate::hooks::{HookContext, HookRegistry, Trigger};

/// Seeds a forest by cloning bare repositories.
pub struct SeedOperation<'a> {
    forest_root: &'a ForestRoot,
    config: &'a ForestConfig,
    hooks: &'a HookRegistry,
    emitter: &'a dyn EventEmitter,
    verbose: bool,
}

impl<'a> SeedOperation<'a> {
    /// Creates a new seed operation.
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

    /// Executes the seed operation.
    pub fn execute(&self) -> Result<(), SeedError> {
        use seed_error::*;

        self.emitter.emit(&ForesterEvent::SeedStarted);

        let bare_dir = self.forest_root.bare_dir();
        std::fs::create_dir_all(&bare_dir).context(CreateDirSnafu { path: &bare_dir })?;

        for member in &self.config.forest.member {
            self.clone_member(member)?;
        }

        let ctx = self.hook_context();
        self.hooks
            .run_hooks(Trigger::PostSeed, &ctx)
            .context(HookSnafu)?;

        self.emitter.emit(&ForesterEvent::SeedCompleted);
        Ok(())
    }

    fn clone_member(&self, member: &crate::domain::Member) -> Result<(), SeedError> {
        use seed_error::*;

        let bare_path = self
            .forest_root
            .bare_dir()
            .join(format!("{}.git", member.name));

        if BareRepository::new(&bare_path, &member.name).exists() {
            return Ok(());
        }

        self.emitter.emit(&ForesterEvent::MemberCloning {
            name: member.name.clone(),
            remote: member.remote.clone(),
        });

        BareRepository::clone_from(&member.remote, &bare_path, !self.verbose)
            .context(CloneSnafu { name: &member.name })?;

        self.emitter.emit(&ForesterEvent::MemberCloned {
            name: member.name.clone(),
        });
        Ok(())
    }

    fn hook_context(&self) -> HookContext {
        HookContext::builder()
            .forest_root(self.forest_root.path())
            .trigger(Trigger::PostSeed)
            .verbose(self.verbose)
            .build()
    }
}

/// Errors from seeding a forest.
#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum SeedError {
    /// Failed to create a directory.
    #[snafu(display("Failed to create directory"))]
    CreateDir {
        /// Path that could not be created.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },

    /// Failed to clone a member repository.
    #[snafu(display("Failed to clone member '{name}'"))]
    Clone {
        /// Member name.
        name: String,
        /// Underlying clone error.
        source: crate::git::CloneBareError,
    },

    /// A hook failed during execution.
    #[snafu(display("Hook failed"))]
    Hook {
        /// Underlying hook error.
        source: crate::hooks::RunHooksError,
    },
}
