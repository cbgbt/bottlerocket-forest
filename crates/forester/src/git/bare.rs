//! Bare git repository operations.

use snafu::{ResultExt, Snafu};
use std::path::{Path, PathBuf};
use std::process::Command;

/// A bare git repository.
#[derive(Debug, Clone)]
pub struct BareRepository {
    path: PathBuf,
    name: String,
}

impl BareRepository {
    /// Creates a new BareRepository reference.
    pub fn new(path: impl Into<PathBuf>, name: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            name: name.into(),
        }
    }

    /// Clones a remote repository as a bare repository.
    pub fn clone_from(remote: &str, path: &Path, quiet: bool) -> Result<Self, CloneBareError> {
        use clone_bare_error::*;

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("repo")
            .to_string();

        let mut cmd = Command::new("git");
        cmd.args(["clone", "--bare", remote]).arg(path);

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

        Ok(Self {
            path: path.to_path_buf(),
            name,
        })
    }

    /// Returns true if this bare repository exists.
    pub fn exists(&self) -> bool {
        self.path.join("HEAD").exists()
    }

    /// Returns the path to the bare repository.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the repository name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Errors from cloning a bare repository.
#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum CloneBareError {
    /// Git clone command failed.
    #[snafu(display("Git clone command failed"))]
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
