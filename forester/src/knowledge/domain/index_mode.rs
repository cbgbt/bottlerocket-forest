//! Index mode for semantic search
//!
//! Currently only supports semantic search with embeddings.

use serde::{Deserialize, Serialize};
use std::fmt;

/// The indexing and search strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndexMode {
    /// Semantic search using embeddings
    Best,
}

impl fmt::Display for IndexMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "best")
    }
}

impl IndexMode {
    /// Returns the display name for user-facing output
    pub fn display_name(&self) -> &'static str {
        "best"
    }

    /// Returns the technical description
    pub fn description(&self) -> &'static str {
        "Semantic search with embeddings"
    }
}
