//! Types for the knowledge index facade

use bon::Builder;
use snafu::Snafu;
use std::time::SystemTime;

use crate::knowledge::domain::{EmbeddingModelConfig, IndexMode};

/// Status information about the knowledge index
#[derive(Debug, Clone, PartialEq, Builder)]
#[non_exhaustive]
pub struct IndexStatus {
    /// Whether the index exists and is accessible
    pub exists: bool,

    /// Index mode (Fast or Best)
    pub mode: IndexMode,

    /// Total number of chunks in the index
    pub chunk_count: usize,

    /// Number of unique files indexed
    pub file_count: usize,

    /// Last time the index was built or updated
    pub last_build: Option<SystemTime>,

    /// Embedding model configuration
    pub model_config: EmbeddingModelConfig,

    /// Approximate size of the index on disk in bytes
    pub size_bytes: Option<u64>,
}

/// Errors that can occur in facade operations
#[derive(Debug, Snafu)]
#[snafu(module, visibility(pub(crate)))]
pub enum IndexError {
    #[snafu(display("Forest root directory not found: {path}"))]
    ForestRootNotFound { path: String },

    #[snafu(display("Failed to create .forester directory"))]
    ForesterDirCreationFailed { source: std::io::Error },

    #[snafu(display("Failed to access database"))]
    DatabaseAccessFailed {
        source: crate::knowledge::storage::StorageError,
    },

    #[snafu(display("Indexing operation failed"))]
    IndexingFailed {
        source: crate::knowledge::indexing::IndexingError,
    },

    #[snafu(display("Search operation failed"))]
    SearchFailed {
        source: crate::knowledge::search::SearchError,
    },

    #[snafu(display("Invalid query: {message}"))]
    InvalidQuery { message: String },

    #[snafu(display("Result limit must be between 1 and 100, got {limit}"))]
    InvalidResultLimit { limit: usize },

    #[snafu(display("Index does not exist. Run `forester index build` to create it."))]
    IndexNotFound,

    #[snafu(display(
        "Index mode mismatch. Index is {index_mode:?} but {requested_mode:?} was requested."
    ))]
    ModeMismatch {
        index_mode: IndexMode,
        requested_mode: IndexMode,
    },

    #[snafu(display("Failed to get database file metadata"))]
    FileMetadataFailed { source: std::io::Error },

    #[snafu(display("Failed to create embedding provider"))]
    EmbeddingProviderCreationFailed {
        source: crate::knowledge::search::embeddings::EmbeddingError,
    },

    #[snafu(display("Failed to create index data provider"))]
    IndexDataProviderCreationFailed {
        source: crate::knowledge::indexing::IndexDataError,
    },
}
