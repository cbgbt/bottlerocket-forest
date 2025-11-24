//! Types for the knowledge index facade
//!
//! This module defines the public API types for interacting with the knowledge index:
//! * [`IndexStatus`] provides metadata about the current state of the index
//! * [`IndexError`] represents all errors that can occur during facade operations
//!
//! These types form the boundary between the high-level facade API and the underlying
//! domain, storage, and search implementations.

use bon::Builder;
use snafu::Snafu;
use std::time::SystemTime;

use crate::knowledge::domain::{EmbeddingModelConfig, QueryTextError, ResultLimitError};

/// Status information about the knowledge index
#[derive(Debug, Clone, PartialEq, Builder)]
#[non_exhaustive]
pub struct IndexStatus {
    /// Whether the index exists and is accessible
    pub exists: bool,

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
#[derive(Debug, Snafu, miette::Diagnostic)]
#[snafu(module, visibility(pub(crate)))]
pub enum IndexError {
    #[snafu(display("Forest root directory not found: {path}"))]
    #[diagnostic(
        code(forester::index::forest_root_not_found),
        help("Ensure you're running the command from within a Bottlerocket forest directory")
    )]
    ForestRootNotFound { path: String },

    #[snafu(display("Failed to create .forester directory for index storage"))]
    #[diagnostic(
        code(forester::index::forester_dir_creation_failed),
        help("Check directory permissions and available disk space")
    )]
    ForesterDirCreationFailed { source: std::io::Error },

    #[snafu(display("Failed to access knowledge index database"))]
    #[diagnostic(
        code(forester::index::database_access_failed),
        help("The database may be corrupted. Try running `forester index rebuild` to recreate it")
    )]
    DatabaseAccessFailed {
        source: crate::knowledge::storage::StorageError,
    },

    #[snafu(display("Failed to index files"))]
    #[diagnostic(
        code(forester::index::indexing_failed),
        help("Check that the files are readable and contain valid content")
    )]
    IndexingFailed {
        source: crate::knowledge::indexing::IndexingError,
    },

    #[snafu(display("Search operation failed"))]
    #[diagnostic(
        code(forester::index::search_failed),
        help("The index may be corrupted or incompatible with the current version")
    )]
    SearchFailed {
        source: crate::knowledge::search::SearchError,
    },

    #[snafu(display("Invalid search query"))]
    #[diagnostic(
        code(forester::index::invalid_query),
        help("Provide a non-empty query string with valid characters")
    )]
    InvalidQuery { source: QueryTextError },

    #[snafu(display("Invalid result limit"))]
    #[diagnostic(
        code(forester::index::invalid_result_limit),
        help("Adjust the --limit parameter to be within the valid range")
    )]
    InvalidResultLimit { source: ResultLimitError },

    #[snafu(display("Failed to read database file metadata"))]
    #[diagnostic(
        code(forester::index::file_metadata_failed),
        help("Check that the database file exists and is accessible")
    )]
    FileMetadataFailed { source: std::io::Error },

    #[snafu(display("Failed to initialize embedding model for semantic search"))]
    #[diagnostic(
        code(forester::index::embedding_provider_creation_failed),
        help("Ensure the model files can be downloaded and cached in .forester/cache/model/")
    )]
    EmbeddingProviderCreationFailed {
        source: crate::knowledge::search::embeddings::EmbeddingError,
    },

    #[snafu(display("Failed to initialize index data provider"))]
    #[diagnostic(
        code(forester::index::index_data_provider_creation_failed),
        help("This is an internal error. Please report this issue")
    )]
    IndexDataProviderCreationFailed {
        source: crate::knowledge::indexing::IndexDataError,
    },

    #[snafu(display("Failed to load forester configuration"))]
    #[diagnostic(
        code(forester::index::config_load_failed),
        help("Check that .forester.toml is valid TOML and contains valid target paths")
    )]
    ConfigLoadFailed {
        source: crate::knowledge::indexing::ForesterConfigError,
    },
}
