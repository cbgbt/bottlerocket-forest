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

use crate::knowledge::domain::{ContextId, EmbeddingModelConfig, QueryTextError, ResultLimitError};

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
        code(sembly::index::forest_root_not_found),
        help("Ensure you're running the command from within a Bottlerocket forest directory")
    )]
    ForestRootNotFound { path: String },

    #[snafu(display("Failed to create .sembly directory for index storage"))]
    #[diagnostic(
        code(sembly::index::sembly_dir_creation_failed),
        help("Check directory permissions and available disk space")
    )]
    SemblyDirCreationFailed { source: std::io::Error },

    #[snafu(display("Failed to access knowledge index database"))]
    #[diagnostic(
        code(sembly::index::database_access_failed),
        help("The database may be corrupted. Try running `sembly rebuild` to recreate it")
    )]
    DatabaseAccessFailed {
        source: crate::knowledge::storage::StorageError,
    },

    #[snafu(display("Failed to index files"))]
    #[diagnostic(
        code(sembly::index::indexing_failed),
        help("Check that the files are readable and contain valid content")
    )]
    IndexingFailed {
        source: crate::knowledge::indexing::IndexingError,
    },

    #[snafu(display("Search operation failed"))]
    #[diagnostic(
        code(sembly::index::search_failed),
        help("The index may be corrupted or incompatible with the current version")
    )]
    SearchFailed {
        source: crate::knowledge::search::SearchError,
    },

    #[snafu(display("Invalid search query"))]
    #[diagnostic(
        code(sembly::index::invalid_query),
        help("Provide a non-empty query string with valid characters")
    )]
    InvalidQuery { source: QueryTextError },

    #[snafu(display("Invalid result limit"))]
    #[diagnostic(
        code(sembly::index::invalid_result_limit),
        help("Adjust the --limit parameter to be within the valid range")
    )]
    InvalidResultLimit { source: ResultLimitError },

    #[snafu(display("Failed to read database file metadata"))]
    #[diagnostic(
        code(sembly::index::file_metadata_failed),
        help("Check that the database file exists and is accessible")
    )]
    FileMetadataFailed { source: std::io::Error },

    #[snafu(display("Failed to initialize embedding model for semantic search"))]
    #[diagnostic(
        code(sembly::index::embedding_provider_creation_failed),
        help("Ensure the model files can be downloaded and cached in .sembly/cache/model/")
    )]
    EmbeddingProviderCreationFailed {
        source: crate::knowledge::search::embeddings::EmbeddingError,
    },

    #[snafu(display("Failed to initialize index data provider"))]
    #[diagnostic(
        code(sembly::index::index_data_provider_creation_failed),
        help("This is an internal error. Please report this issue")
    )]
    IndexDataProviderCreationFailed {
        source: crate::knowledge::indexing::IndexDataError,
    },

    #[snafu(display("Failed to load sembly configuration"))]
    #[diagnostic(
        code(sembly::index::config_load_failed),
        help("Check that .sembly.toml is valid TOML and contains valid target paths")
    )]
    ConfigLoadFailed {
        source: crate::knowledge::indexing::SemblyConfigError,
    },

    #[snafu(display("Index already exists at {path}"))]
    #[diagnostic(
        code(sembly::index::already_exists),
        help("Use 'sembly rebuild' to recreate the index or 'sembly update' to refresh it")
    )]
    IndexAlreadyExists { path: String },

    #[snafu(display("Index does not exist at {path}"))]
    #[diagnostic(
        code(sembly::index::not_found),
        help("Use 'sembly build' to create the index")
    )]
    IndexNotFound { path: String },

    #[snafu(display("Failed to delete index database"))]
    #[diagnostic(
        code(sembly::index::deletion_failed),
        help("Check file permissions and ensure the database is not in use")
    )]
    IndexDeletionFailed { source: std::io::Error },

    #[snafu(display("No sembly workspace found"))]
    #[diagnostic(
        code(sembly::index::workspace_not_found),
        help(
            "Run `sembly build` to create an index, or navigate to a directory within an existing workspace"
        )
    )]
    WorkspaceNotFound {
        source: crate::knowledge::context::DiscoveryError,
    },

    #[snafu(display("No matching context for current directory"))]
    #[diagnostic(
        code(sembly::index::context_not_found),
        help("Available contexts: {}", available_contexts.iter().map(|c| c.as_str()).collect::<Vec<_>>().join(", "))
    )]
    ContextNotFound { available_contexts: Vec<ContextId> },

    #[snafu(display("Failed to resolve context"))]
    #[diagnostic(
        code(sembly::index::context_resolution_failed),
        help("Ensure you are within a registered context directory")
    )]
    ContextResolutionFailed {
        source: crate::knowledge::context::ResolutionError,
    },

    #[snafu(display("Failed to register context"))]
    #[diagnostic(
        code(sembly::index::context_registration_failed),
        help("The context may already exist or the database may be inaccessible")
    )]
    ContextRegistrationFailed {
        source: crate::knowledge::storage::ContextRepositoryError,
    },
}
