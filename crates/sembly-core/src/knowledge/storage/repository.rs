//! Abstract repository interface for chunk persistence
//!
//! Defines the contract for storing, retrieving, and searching indexed documentation chunks.

use snafu::Snafu;
use std::collections::HashSet;

use crate::knowledge::domain::{
    ChunkHash, ChunkId, Context, ContextId, EmbeddingModelConfig, FileHash, ForestRelativePath,
    IndexMetadata, IndexedChunk, Timestamp,
};

/// Abstract interface for chunk storage operations
#[cfg_attr(test, mockall::automock)]
pub trait ChunkRepository {
    /// Persists a single indexed chunk to storage
    fn save(&mut self, chunk: &IndexedChunk) -> Result<(), StorageError>;

    /// Persists multiple indexed chunks in a single transaction
    fn save_batch(&mut self, chunks: &[IndexedChunk]) -> Result<(), StorageError>;

    /// Retrieves an indexed chunk by its unique identifier
    fn find_by_id(&self, id: &ChunkId) -> Result<Option<IndexedChunk>, StorageError>;

    /// Retrieves all indexed chunks from a specific file
    fn find_by_file(&self, path: &ForestRelativePath) -> Result<Vec<IndexedChunk>, StorageError>;

    /// Retrieves all indexed chunks from storage
    fn find_all(&self) -> Result<Vec<IndexedChunk>, StorageError>;

    /// Retrieves file paths and their most recent indexing timestamps
    ///
    /// More efficient than `find_all()` for incremental update comparisons.
    fn get_indexed_files(
        &self,
    ) -> Result<
        std::collections::HashMap<ForestRelativePath, crate::knowledge::domain::Timestamp>,
        StorageError,
    >;

    /// Removes all chunks associated with a specific file
    fn delete_by_file(&mut self, path: &ForestRelativePath) -> Result<usize, StorageError>;

    /// Removes all chunks from storage
    fn clear(&mut self) -> Result<usize, StorageError>;

    /// Retrieves index metadata including build time and model configuration
    fn get_metadata(&self) -> Result<IndexMetadata, StorageError>;

    /// Updates index metadata
    fn set_metadata(&mut self, metadata: &IndexMetadata) -> Result<(), StorageError>;

    /// Searches for chunks semantically similar to the query embedding
    ///
    /// Returns chunks ranked by similarity score in descending order.
    /// When context_id is Some, results are filtered to files in that context.
    /// When context_id is None, all chunks are searched.
    fn search_semantic(
        &self,
        query_embedding: &[f32],
        limit: usize,
        context_id: Option<ContextId>,
    ) -> Result<Vec<(IndexedChunk, f32)>, StorageError>;

    /// Checks if an embedding exists for the given chunk hash
    fn has_embedding(&self, chunk_hash: &ChunkHash) -> Result<bool, StorageError>;

    /// Checks which chunk hashes already have embeddings
    ///
    /// Returns the subset of input hashes that have existing embeddings.
    fn has_embedding_batch(
        &self,
        chunk_hashes: &[ChunkHash],
    ) -> Result<HashSet<ChunkHash>, StorageError>;

    /// Records a file as indexed in the default context
    ///
    /// Creates or updates an indexed_files record linking the file to the default context.
    fn track_indexed_file(
        &mut self,
        file_path: &ForestRelativePath,
        file_hash: &FileHash,
        mtime: Timestamp,
    ) -> Result<(), StorageError>;
}

/// Abstract interface for context storage operations
///
/// Manages the lifecycle of contexts and their file mappings in multi-context indexing.
/// Contexts represent registered working directories that share a common embedding database.
#[cfg_attr(test, mockall::automock)]
pub trait ContextRepository {
    /// Retrieves all registered contexts
    fn list_contexts(&self) -> Result<Vec<Context>, ContextRepositoryError>;

    /// Retrieves a specific context by its identifier
    fn get_context(
        &self,
        context_id: &ContextId,
    ) -> Result<Option<Context>, ContextRepositoryError>;

    /// Registers a new context in the workspace
    fn insert_context(&self, context: &Context) -> Result<(), ContextRepositoryError>;

    /// Removes a context and its file mappings
    fn remove_context(&self, context_id: &ContextId) -> Result<(), ContextRepositoryError>;
}

#[derive(Debug, Snafu, miette::Diagnostic)]
#[snafu(module, visibility(pub))]
pub enum ContextRepositoryError {
    #[snafu(display("Database operation failed"))]
    #[diagnostic(
        code(sembly::context::database_error),
        help("The database may be locked, corrupted, or out of disk space")
    )]
    DatabaseError { source: rusqlite::Error },

    #[snafu(display("Context not found: {context_id}"))]
    #[diagnostic(
        code(sembly::context::not_found),
        help("Use `sembly context list` to see available contexts")
    )]
    NotFound { context_id: String },

    #[snafu(display("Context already exists: {context_id}"))]
    #[diagnostic(
        code(sembly::context::already_exists),
        help("Use a different path or remove the existing context first")
    )]
    AlreadyExists { context_id: String },
}

#[derive(Debug, Snafu, miette::Diagnostic)]
#[snafu(module, visibility(pub))]
pub enum StorageError {
    #[snafu(display("Database operation failed"))]
    #[diagnostic(
        code(sembly::storage::database_error),
        help("The database may be locked, corrupted, or out of disk space")
    )]
    DatabaseError { source: rusqlite::Error },

    #[snafu(display("Failed to serialize chunk data to JSON"))]
    #[diagnostic(
        code(sembly::storage::serialization_error),
        help("The chunk may contain invalid UTF-8 or unsupported characters")
    )]
    SerializationError { source: serde_json::Error },

    #[snafu(display("Invalid data in database: {message}"))]
    #[diagnostic(
        code(sembly::storage::invalid_data),
        help("The database may be corrupted. Try running `sembly rebuild`")
    )]
    InvalidData { message: String },

    #[snafu(display("Invalid value in database field '{field}'"))]
    #[diagnostic(
        code(sembly::storage::invalid_field),
        help("The database schema may be incompatible with this version")
    )]
    InvalidField {
        field: String,
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },

    #[snafu(display("Chunk not found in index: {id:?}"))]
    #[diagnostic(
        code(sembly::storage::not_found),
        help("The chunk may have been deleted or the index may be out of sync")
    )]
    NotFound { id: ChunkId },

    #[snafu(display("Operation not supported in current index mode: {operation}"))]
    #[diagnostic(
        code(sembly::storage::unsupported_operation),
        help("This operation requires a different index mode")
    )]
    UnsupportedOperation { operation: String },

    #[snafu(display("Index configuration mismatch\nExpected: {expected:?}\nFound: {actual:?}"))]
    #[diagnostic(
        code(sembly::storage::config_mismatch),
        help("Run `sembly rebuild` to recreate the index with the current configuration")
    )]
    ConfigMismatch {
        expected: EmbeddingModelConfig,
        actual: EmbeddingModelConfig,
    },
}
