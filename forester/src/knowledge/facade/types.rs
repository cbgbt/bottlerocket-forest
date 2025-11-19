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

    #[snafu(display("Invalid search query: {message}"))]
    #[diagnostic(
        code(forester::index::invalid_query),
        help("Provide a non-empty query string with valid characters")
    )]
    InvalidQuery { message: String },

    #[snafu(display("Result limit must be between 1 and 100, got {limit}"))]
    #[diagnostic(
        code(forester::index::invalid_result_limit),
        help("Adjust the --limit parameter to be within the valid range")
    )]
    InvalidResultLimit { limit: usize },

    #[snafu(display("Knowledge index does not exist"))]
    #[diagnostic(
        code(forester::index::index_not_found),
        help("Run `forester index build` to create the index before searching")
    )]
    IndexNotFound,

    #[snafu(display(
        "Index mode mismatch: index uses {index_mode:?} mode but {requested_mode:?} mode was requested"
    ))]
    #[diagnostic(
        code(forester::index::mode_mismatch),
        help(
            "Run `forester index rebuild --mode {requested_mode:?}` to recreate the index in the desired mode"
        )
    )]
    ModeMismatch {
        index_mode: IndexMode,
        requested_mode: IndexMode,
    },

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
}
