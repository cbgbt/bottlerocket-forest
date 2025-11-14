//! Repository trait for chunk persistence

use bon::Builder;
use serde::{Deserialize, Serialize};
use snafu::Snafu;

use crate::knowledge::domain::{Chunk, ChunkId, ForestRelativePath, IndexMode};

/// Repository for chunk persistence
#[cfg_attr(test, mockall::automock)]
pub trait ChunkRepository {
    /// Store a chunk
    fn save(&mut self, chunk: &Chunk) -> Result<(), StorageError>;

    /// Store multiple chunks (transaction)
    fn save_batch(&mut self, chunks: &[Chunk]) -> Result<(), StorageError>;

    /// Retrieve a chunk by ID
    fn find_by_id(&self, id: &ChunkId) -> Result<Option<Chunk>, StorageError>;

    /// Find all chunks for a file
    fn find_by_file(&self, path: &ForestRelativePath) -> Result<Vec<Chunk>, StorageError>;

    /// Find all chunks in the index
    fn find_all(&self) -> Result<Vec<Chunk>, StorageError>;

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
    /// Returns chunks ranked by similarity to the query embedding, with scores.
    fn search_semantic(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<(Chunk, f32)>, StorageError>;

    /// Search chunks using BM25 keyword matching
    ///
    /// Returns chunks ranked by BM25 relevance to the query terms, with scores.
    fn search_bm25(
        &self,
        query_terms: &[String],
        limit: usize,
    ) -> Result<Vec<(Chunk, f32)>, StorageError>;
}

/// Metadata about the index
#[derive(Debug, Clone, PartialEq, Builder, Serialize, Deserialize)]
#[non_exhaustive]
pub struct IndexMetadata {
    pub mode: IndexMode,
    pub last_build: std::time::SystemTime,
    pub chunk_count: usize,
    pub file_count: usize,
}

/// Errors that can occur during storage operations
#[derive(Debug, Snafu)]
#[snafu(module, visibility(pub))]
pub enum StorageError {
    #[snafu(display("Database error"))]
    DatabaseError { source: rusqlite::Error },

    #[snafu(display("Failed to serialize data"))]
    SerializationError { source: serde_json::Error },

    #[snafu(display("Invalid data: {message}"))]
    InvalidData { message: String },

    #[snafu(display("Invalid field '{field}'"))]
    InvalidField {
        field: String,
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },

    #[snafu(display("Chunk not found: {id:?}"))]
    NotFound { id: ChunkId },

    #[snafu(display("Operation not supported: {operation}"))]
    UnsupportedOperation { operation: String },
}
