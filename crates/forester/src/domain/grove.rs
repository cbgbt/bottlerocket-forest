//! Grove domain types for forester.

use std::path::{Path, PathBuf};

use bon::Builder;
use nutype::nutype;

use snafu::Snafu;

#[nutype(
    derive(Debug, Clone, PartialEq, Eq, Hash, Display, Serialize, Deserialize),
    validate(with = validate_grove_name, error = GroveNameError)
)]
pub struct GroveName(String);

fn validate_grove_name(raw: &str) -> Result<(), GroveNameError> {
    use grove_name_error::*;
    snafu::ensure!(!raw.is_empty(), EmptySnafu);
    snafu::ensure!(
        raw.chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_'),
        InvalidCharSnafu
    );
    Ok(())
}

#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum GroveNameError {
    #[snafu(display("Grove name cannot be empty"))]
    Empty,
    #[snafu(display(
        "Grove name contains invalid characters (only alphanumeric, hyphens, underscores allowed)"
    ))]
    InvalidChar,
}

/// Root directory of a grove.
#[derive(Debug, Clone, PartialEq, Eq, Builder)]
#[non_exhaustive]
pub struct GroveRoot {
    #[builder(into)]
    path: PathBuf,
}

impl GroveRoot {
    /// Returns the grove path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the grove marker directory path.
    pub fn marker_dir(&self) -> PathBuf {
        self.path.join(".grove")
    }
}
