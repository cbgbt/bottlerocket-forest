//! Repository trait for chunk persistence

use bon::Builder;
use serde::{Deserialize, Serialize};
use snafu::Snafu;
use std::collections::BTreeMap;

use crate::knowledge::constants;
use crate::knowledge::domain::{Chunk, ChunkId, Embedding, ForestRelativePath, IndexMode};

/// Configuration for embedding model and chunking parameters
///
/// These parameters are fundamental to the index structure. If any of these
/// values change, the entire index must be rebuilt.
#[derive(Debug, Clone, PartialEq, Builder, Serialize, Deserialize)]
#[non_exhaustive]
pub struct EmbeddingModelConfig {
    #[builder(into)]
    pub model_name: String,
    pub embedding_dim: usize,
    pub max_tokens: usize,
    pub overlap_tokens: usize,
}

impl Default for EmbeddingModelConfig {
    fn default() -> Self {
        Self {
            model_name: constants::DEFAULT_TOKENIZER_MODEL.to_string(),
            embedding_dim: constants::EMBEDDING_DIM,
            max_tokens: constants::DEFAULT_MAX_CHUNK_TOKENS,
            overlap_tokens: constants::DEFAULT_CHUNK_OVERLAP_TOKENS,
        }
    }
}

/// A chunk with index-specific metadata
///
/// Wraps a domain `Chunk` with indexing data (embeddings or BM25 terms) and
/// a timestamp indicating when it was indexed.
#[derive(Debug, Clone, Builder)]
#[non_exhaustive]
pub struct IndexedChunk {
    pub chunk: Chunk,
    pub index_data: IndexData,
    pub indexed_at: Timestamp,
}

/// Index-specific data for a chunk
///
/// Represents either BM25 term frequencies (Fast mode) or semantic embeddings
/// (Best mode). This type ensures chunks are indexed with the correct data
/// for their mode.
#[derive(Debug, Clone)]
pub enum IndexData {
    Fast { bm25_terms: BTreeMap<String, u32> },
    Best { embedding: Embedding },
}

impl IndexData {
    /// Returns the index mode for this data
    pub fn mode(&self) -> IndexMode {
        match self {
            IndexData::Fast { .. } => IndexMode::Fast,
            IndexData::Best { .. } => IndexMode::Best,
        }
    }
}

/// Unix timestamp in seconds
///
/// Represents when a chunk was indexed, used for staleness detection
/// and incremental updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp(i64);

impl Timestamp {
    /// Create a timestamp from Unix seconds
    pub fn from_secs(secs: i64) -> Self {
        Self(secs)
    }

    /// Get the Unix seconds value
    pub fn as_secs(&self) -> i64 {
        self.0
    }

    /// Create a timestamp for the current time
    pub fn now() -> Self {
        Self(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time before Unix epoch")
                .as_secs() as i64,
        )
    }
}

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
    pub model_config: EmbeddingModelConfig,
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

    #[snafu(display(
        "Index configuration mismatch. Expected: {expected:?}, Found: {actual:?}. Run `forester index rebuild` to recreate the index with the current configuration."
    ))]
    ConfigMismatch {
        expected: EmbeddingModelConfig,
        actual: EmbeddingModelConfig,
    },
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_index_data_mode_returns_correct_mode() {
        // Given Fast mode index data
        let fast_data = IndexData::Fast {
            bm25_terms: BTreeMap::new(),
        };

        // When Getting the mode
        // Then It should return Fast
        assert_eq!(fast_data.mode(), IndexMode::Fast);

        // Given Best mode index data
        let best_data = IndexData::Best {
            embedding: Embedding::try_new(vec![0.1, 0.2, 0.3]).unwrap(),
        };

        // When Getting the mode
        // Then It should return Best
        assert_eq!(best_data.mode(), IndexMode::Best);
    }
}
