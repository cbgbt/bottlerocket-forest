//! Grove listing operation.

use std::path::PathBuf;

use snafu::{ResultExt, Snafu};

use crate::domain::{ForestRoot, GroveName};
use crate::events::{EventEmitter, ForesterEvent};

/// Information about a grove.
#[derive(Debug, Clone)]
pub struct GroveInfo {
    /// Grove name.
    pub name: GroveName,
    /// Path to the grove.
    pub path: PathBuf,
    /// Whether this is the current grove.
    pub is_current: bool,
}

/// Lists groves in the forest.
pub struct GroveListOperation<'a> {
    forest_root: &'a ForestRoot,
    emitter: &'a dyn EventEmitter,
    current_dir: Option<PathBuf>,
}

impl<'a> GroveListOperation<'a> {
    /// Creates a new grove list operation.
    pub fn new(
        forest_root: &'a ForestRoot,
        emitter: &'a dyn EventEmitter,
        current_dir: Option<PathBuf>,
    ) -> Self {
        Self {
            forest_root,
            emitter,
            current_dir,
        }
    }

    /// Executes the grove listing.
    pub fn execute(&self) -> Result<Vec<GroveInfo>, GroveListError> {
        use grove_list_error::*;

        self.emitter.emit(&ForesterEvent::GroveListing);

        let groves_dir = self.forest_root.groves_dir();
        if !groves_dir.exists() {
            return Ok(vec![]);
        }

        let mut groves = Vec::new();
        let entries = std::fs::read_dir(&groves_dir).context(ReadDirSnafu { path: &groves_dir })?;

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let file_name = entry.file_name();
            let Some(name_str) = file_name.to_str() else {
                continue;
            };
            let Ok(name) = GroveName::try_new(name_str.to_string()) else {
                continue;
            };

            let is_current = self
                .current_dir
                .as_ref()
                .map(|cwd| cwd.starts_with(&path))
                .unwrap_or(false);

            groves.push(GroveInfo {
                name,
                path,
                is_current,
            });
        }

        groves.sort_by(|a, b| a.name.to_string().cmp(&b.name.to_string()));
        Ok(groves)
    }
}

/// Errors from listing groves.
#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum GroveListError {
    /// Failed to read the groves directory.
    #[snafu(display("Failed to read groves directory"))]
    ReadDir {
        /// Path that could not be read.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
}
