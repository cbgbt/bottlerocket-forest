//! Search engine abstraction for knowledge index queries

use snafu::Snafu;

use crate::knowledge::domain::{IndexMode, SearchQuery, SearchResults};

/// Search engine for querying the knowledge index
#[cfg_attr(test, mockall::automock)]
pub trait SearchEngine {
    /// Execute a search query
    fn search(&self, query: &SearchQuery) -> Result<SearchResults, SearchError>;

    /// Get the index mode this engine supports
    fn mode(&self) -> IndexMode;
}

/// Errors that can occur during search operations
#[derive(Debug, Snafu)]
#[snafu(module, visibility(pub(crate)))]
pub enum SearchError {
    #[snafu(display("Storage operation failed"))]
    Storage {
        source: crate::knowledge::storage::StorageError,
    },

    #[snafu(display("Query mode {query_mode:?} does not match engine mode {engine_mode:?}"))]
    ModeMismatch {
        query_mode: IndexMode,
        engine_mode: IndexMode,
    },

    #[snafu(display("Failed to generate query embedding"))]
    EmbeddingFailed {
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },

    #[snafu(display("Repository returned invalid relevance score: {score}"))]
    InvalidScore {
        score: f32,
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
}
