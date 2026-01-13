//! Grove context detection and access.

use crate::domain::ForestConfig;
use bon::Builder;
use snafu::{ResultExt, Snafu};
use std::path::PathBuf;

/// Detects and provides access to the current grove context.
#[derive(Debug, Clone, PartialEq, Eq, Builder)]
#[builder(on(_, into))]
#[non_exhaustive]
pub struct GroveContext {
    forest_root: PathBuf,
    grove_root: PathBuf,
    name: String,
}

impl GroveContext {
    /// Detects the current grove by checking if cwd is under a forest's groves directory.
    pub fn detect() -> Result<Option<Self>, GroveContextError> {
        use grove_context_error::*;

        let (forest_root, _config) = find_forest_config().context(FindForestSnafu)?;
        let cwd = std::env::current_dir().context(CurrentDirSnafu)?;
        let groves_dir = forest_root.join("groves");

        let rel = match cwd.strip_prefix(&groves_dir) {
            Ok(r) => r,
            Err(_) => return Ok(None),
        };

        let name = rel
            .components()
            .next()
            .and_then(|c| c.as_os_str().to_str())
            .map(String::from)
            .ok_or(GroveContextError::InvalidName)?;

        let grove_root = groves_dir.join(&name);
        if !grove_root.join(".grove").is_dir() {
            return Ok(None);
        }

        Ok(Some(Self {
            forest_root,
            grove_root,
            name,
        }))
    }

    /// Returns the forest root directory path.
    pub fn forest_root(&self) -> &PathBuf {
        &self.forest_root
    }

    /// Returns the grove root directory path.
    pub fn grove_root(&self) -> &PathBuf {
        &self.grove_root
    }

    /// Returns the grove name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Errors that can occur when resolving grove context.
#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum GroveContextError {
    /// Failed to locate the forest root directory.
    #[snafu(display("Failed to find forest root"))]
    FindForest {
        /// Underlying error.
        source: FindForestError,
    },

    /// Failed to determine current working directory.
    #[snafu(display("Failed to get current directory"))]
    CurrentDir {
        /// Underlying I/O error.
        source: std::io::Error,
    },

    /// Grove directory name is not valid UTF-8.
    #[snafu(display("Grove directory has invalid name"))]
    InvalidName,
}

/// Error finding forest configuration.
#[derive(Debug, Snafu)]
pub enum FindForestError {
    #[snafu(display("Failed to get current directory"))]
    CurrentDir { source: std::io::Error },
    #[snafu(display("No forester.toml found in current directory or parents"))]
    NotFound,
}

fn find_forest_config() -> Result<(PathBuf, ForestConfig), FindForestError> {
    let cwd = std::env::current_dir().context(CurrentDirSnafu)?;
    let mut dir = cwd;
    loop {
        let config_path = dir.join("forester.toml");
        if config_path.exists()
            && let Ok(content) = std::fs::read_to_string(&config_path)
            && let Ok(config) = toml::from_str(&content)
        {
            return Ok((dir, config));
        }
        if !dir.pop() {
            return Err(FindForestError::NotFound);
        }
    }
}
