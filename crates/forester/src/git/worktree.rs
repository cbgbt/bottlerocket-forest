//! Git worktree operations.

use snafu::{ResultExt, Snafu};
use std::path::Path;
use std::process::Command;

use super::BareRepository;

impl BareRepository {
    /// Creates a worktree at the target path for an existing branch.
    pub fn create_worktree(
        &self,
        target: &Path,
        branch: &str,
        quiet: bool,
    ) -> Result<(), CreateWorktreeError> {
        use create_worktree_error::*;

        let mut cmd = Command::new("git");
        cmd.current_dir(self.path())
            .args(["worktree", "add"])
            .arg(target)
            .arg(branch);

        if quiet {
            cmd.stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
        }

        let status = cmd.status().context(IoSnafu)?;
        snafu::ensure!(
            status.success(),
            CommandFailedSnafu {
                exit_code: status.code()
            }
        );

        Ok(())
    }

    /// Creates a worktree at the target path with a new branch.
    pub fn create_worktree_new_branch(
        &self,
        target: &Path,
        new_branch: &str,
        start_point: &str,
        quiet: bool,
    ) -> Result<(), CreateWorktreeError> {
        use create_worktree_error::*;

        let mut cmd = Command::new("git");
        cmd.current_dir(self.path())
            .args(["worktree", "add", "-b", new_branch])
            .arg(target)
            .arg(start_point);

        if quiet {
            cmd.stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
        }

        let status = cmd.status().context(IoSnafu)?;
        snafu::ensure!(
            status.success(),
            CommandFailedSnafu {
                exit_code: status.code()
            }
        );

        Ok(())
    }

    /// Removes a worktree at the target path.
    pub fn remove_worktree(&self, target: &Path, force: bool) -> Result<(), RemoveWorktreeError> {
        use remove_worktree_error::*;

        let mut cmd = Command::new("git");
        cmd.current_dir(self.path())
            .args(["worktree", "remove"])
            .arg(target);

        if force {
            cmd.arg("--force");
        }

        let status = cmd.status().context(IoSnafu)?;
        snafu::ensure!(
            status.success(),
            CommandFailedSnafu {
                exit_code: status.code()
            }
        );

        Ok(())
    }
}

/// Errors from creating a worktree.
#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum CreateWorktreeError {
    /// Git worktree add command failed.
    #[snafu(display("Git worktree add command failed"))]
    CommandFailed {
        /// The exit code from git.
        exit_code: Option<i32>,
    },

    /// Failed to execute git.
    #[snafu(display("Failed to execute git"))]
    Io {
        /// The underlying IO error.
        source: std::io::Error,
    },
}

/// Errors from removing a worktree.
#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum RemoveWorktreeError {
    /// Git worktree remove command failed.
    #[snafu(display("Git worktree remove command failed"))]
    CommandFailed {
        /// The exit code from git.
        exit_code: Option<i32>,
    },

    /// Failed to execute git.
    #[snafu(display("Failed to execute git"))]
    Io {
        /// The underlying IO error.
        source: std::io::Error,
    },
}
