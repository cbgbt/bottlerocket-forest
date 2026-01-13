//! Error types for forester operations.

use miette::Diagnostic;
use snafu::Snafu;
use std::path::PathBuf;

/// Top-level error type for forester operations.
#[derive(Debug, Snafu, Diagnostic)]
#[snafu(visibility(pub))]
pub enum Error {
    /// Failed to read a config file.
    #[snafu(display("Failed to read config from {}", path.display()))]
    ConfigRead {
        /// Path to the config file.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },

    /// Failed to parse a config file.
    #[snafu(display("Failed to parse config from {}", path.display()))]
    ConfigParse {
        /// Path to the config file.
        path: PathBuf,
        /// Underlying parse error.
        source: toml::de::Error,
    },

    /// Failed to get the current directory.
    #[snafu(display("Failed to get current directory"))]
    CurrentDir {
        /// Underlying IO error.
        source: std::io::Error,
    },

    /// No forester.toml found.
    #[snafu(display("No forester.toml found in current directory or parents"))]
    NoForestFound,

    /// Failed to create a directory.
    #[snafu(display("Failed to create directory {}", path.display()))]
    CreateDir {
        /// Path that could not be created.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },

    /// A git operation failed.
    #[snafu(display("Git operation failed: {message}"))]
    Git {
        /// Error message.
        message: String,
    },

    /// Grove not found.
    #[snafu(display("Grove '{name}' not found"))]
    GroveNotFound {
        /// Grove name.
        name: String,
    },

    /// Failed to create a symlink.
    #[snafu(display("Failed to create symlink from {} to {}", src.display(), tgt.display()))]
    Symlink {
        /// Source path.
        src: PathBuf,
        /// Target path.
        tgt: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
}
