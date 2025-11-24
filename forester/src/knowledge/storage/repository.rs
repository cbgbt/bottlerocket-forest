//! Abstract repository interface for chunk persistence
//!
//! Defines the contract for storing, retrieving, and searching indexed documentation chunks.

use snafu::Snafu;

use crate::knowledge::domain::{
    ChunkId, EmbeddingModelConfig, ForestRelativePath, IndexMetadata, IndexedChunk,
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
    fn search_semantic(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<(IndexedChunk, f32)>, StorageError>;
}

#[derive(Debug, Snafu, miette::Diagnostic)]
#[snafu(module, visibility(pub))]
pub enum StorageError {
    #[snafu(display("Database operation failed"))]
    #[diagnostic(
        code(forester::storage::database_error),
        help("The database may be locked, corrupted, or out of disk space")
    )]
    DatabaseError { source: rusqlite::Error },

    #[snafu(display("Failed to serialize chunk data to JSON"))]
    #[diagnostic(
        code(forester::storage::serialization_error),
        help("The chunk may contain invalid UTF-8 or unsupported characters")
    )]
    SerializationError { source: serde_json::Error },

    #[snafu(display("Invalid data in database: {message}"))]
    #[diagnostic(
        code(forester::storage::invalid_data),
        help("The database may be corrupted. Try running `forester index rebuild`")
    )]
    InvalidData { message: String },

    #[snafu(display("Invalid value in database field '{field}'"))]
    #[diagnostic(
        code(forester::storage::invalid_field),
        help("The database schema may be incompatible with this version")
    )]
    InvalidField {
        field: String,
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },

    #[snafu(display("Chunk not found in index: {id:?}"))]
    #[diagnostic(
        code(forester::storage::not_found),
        help("The chunk may have been deleted or the index may be out of sync")
    )]
    NotFound { id: ChunkId },

    #[snafu(display("Operation not supported in current index mode: {operation}"))]
    #[diagnostic(
        code(forester::storage::unsupported_operation),
        help("This operation requires a different index mode")
    )]
    UnsupportedOperation { operation: String },

    #[snafu(display("Index configuration mismatch\nExpected: {expected:?}\nFound: {actual:?}"))]
    #[diagnostic(
        code(forester::storage::config_mismatch),
        help("Run `forester index rebuild` to recreate the index with the current configuration")
    )]
    ConfigMismatch {
        expected: EmbeddingModelConfig,
        actual: EmbeddingModelConfig,
    },
}
