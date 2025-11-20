//! Repository trait for chunk persistence

use snafu::Snafu;

use crate::knowledge::domain::{
    ChunkId, EmbeddingModelConfig, ForestRelativePath, IndexMetadata, IndexedChunk,
};

/// Repository for chunk persistence
#[cfg_attr(test, mockall::automock)]
pub trait ChunkRepository {
    /// Store an indexed chunk
    fn save(&mut self, chunk: &IndexedChunk) -> Result<(), StorageError>;

    /// Store multiple indexed chunks (transaction)
    fn save_batch(&mut self, chunks: &[IndexedChunk]) -> Result<(), StorageError>;

    /// Retrieve an indexed chunk by ID
    fn find_by_id(&self, id: &ChunkId) -> Result<Option<IndexedChunk>, StorageError>;

    /// Find all indexed chunks for a file
    fn find_by_file(&self, path: &ForestRelativePath) -> Result<Vec<IndexedChunk>, StorageError>;

    /// Find all indexed chunks in the index
    fn find_all(&self) -> Result<Vec<IndexedChunk>, StorageError>;

    /// Get indexed file metadata (path and last indexed timestamp)
    ///
    /// Returns a map of file paths to their most recent indexing timestamp.
    /// This is more efficient than `find_all()` for incremental update comparisons.
    fn get_indexed_files(
        &self,
    ) -> Result<
        std::collections::HashMap<ForestRelativePath, crate::knowledge::domain::Timestamp>,
        StorageError,
    >;

    /// Remove chunks for a file
    fn delete_by_file(&mut self, path: &ForestRelativePath) -> Result<usize, StorageError>;

    /// Clear all chunks
    fn clear(&mut self) -> Result<usize, StorageError>;

    /// Get index metadata
    fn get_metadata(&self) -> Result<IndexMetadata, StorageError>;

    /// Update index metadata
    fn set_metadata(&mut self, metadata: &IndexMetadata) -> Result<(), StorageError>;

    /// Search chunks using semantic similarity
    ///
    /// Returns indexed chunks ranked by similarity to the query embedding, with scores.
    fn search_semantic(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<(IndexedChunk, f32)>, StorageError>;
}

/// Errors that can occur during storage operations
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
