//! Search engine abstraction for knowledge index queries
//!
//! Defines the core search interface and error types used by all search implementations.

use snafu::Snafu;

use crate::knowledge::domain::{SearchQuery, SearchResults};

/// Executes search queries against the knowledge index
#[cfg_attr(test, mockall::automock)]
pub trait SearchEngine {
    /// Execute a search query against the knowledge index
    fn search(&self, query: &SearchQuery) -> Result<SearchResults, SearchError>;
}

/// Errors that occur during search operations
#[derive(Debug, Snafu, miette::Diagnostic)]
#[snafu(module, visibility(pub(crate)))]
pub enum SearchError {
    #[snafu(display("Failed to query database during search"))]
    #[diagnostic(
        code(forester::search::storage_error),
        help("The database may be locked or corrupted")
    )]
    Storage {
        source: crate::knowledge::storage::StorageError,
    },

    #[snafu(display("Failed to generate embedding vector for search query"))]
    #[diagnostic(
        code(forester::search::embedding_failed),
        help("The embedding model may not be loaded or the query text may be invalid")
    )]
    EmbeddingFailed {
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },

    #[snafu(display("Database returned invalid relevance score: {score}"))]
    #[diagnostic(
        code(forester::search::invalid_score),
        help("The index may be corrupted. Try running `forester index rebuild`")
    )]
    InvalidScore {
        score: f32,
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
}
