//! Forest identity and root path types.

use std::path::PathBuf;

use bon::Builder;
use nutype::nutype;


#[nutype(validate(not_empty), derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize))]
pub struct ForestName(String);

/// Root directory of a forest.
#[derive(Debug, Clone, PartialEq, Eq, Builder)]
#[non_exhaustive]
pub struct ForestRoot {
    #[builder(into)]
    path: PathBuf,
}

impl ForestRoot {
    /// Returns the forest root path.
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Directory containing bare repositories.
    pub fn bare_dir(&self) -> PathBuf {
        self.path.join(".forest").join("bare")
    }

    /// Directory containing groves.
    pub fn groves_dir(&self) -> PathBuf {
        self.path.join("groves")
    }

    /// Path to the forest configuration file.
    pub fn config_path(&self) -> PathBuf {
        self.path.join("forester.toml")
    }
}
