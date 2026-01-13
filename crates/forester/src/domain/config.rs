//! Configuration types for forest and grove definitions.

use bon::Builder;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Forest configuration loaded from `forester.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Builder, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
#[non_exhaustive]
pub struct ForestConfig {
    /// Core forest metadata.
    pub forest: ForestMeta,
    /// Optional grove-specific configuration.
    #[serde(default)]
    pub grove: Option<GroveConfig>,
}

/// Forest metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ForestMeta {
    /// Forest name.
    pub name: String,
    /// Member repositories.
    #[serde(default)]
    pub member: Vec<Member>,
}

/// Grove configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct GroveConfig {
    /// Symlinks to create in groves.
    #[serde(default)]
    pub symlink: Vec<SymlinkEntry>,
}

/// A symlink to create in groves.
#[derive(Debug, Clone, PartialEq, Eq, Builder, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SymlinkEntry {
    /// Source path relative to forest root.
    #[builder(into)]
    pub source: PathBuf,
    /// Target path relative to grove root.
    #[builder(into)]
    pub target: PathBuf,
}

/// A member repository in the forest.
#[derive(Debug, Clone, PartialEq, Eq, Builder, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Member {
    /// Identifier for this member repository.
    #[builder(into)]
    pub name: String,
    /// Git remote URL.
    #[builder(into)]
    pub remote: String,
    /// Relative path within the grove.
    #[builder(into)]
    pub path: PathBuf,
    /// Branch to check out by default.
    #[serde(default)]
    pub default_branch: Option<String>,
}

impl Member {
    /// Returns the default branch, falling back to "main".
    pub fn branch(&self) -> &str {
        self.default_branch.as_deref().unwrap_or("main")
    }
}
