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
}

/// Metadata about the index
#[derive(Debug, Clone, Builder, Serialize, Deserialize)]
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

    #[snafu(display("Chunk not found: {id:?}"))]
    NotFound { id: ChunkId },
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_mock_repository() {
        // Given A mock repository
        let mut mock = MockChunkRepository::new();

        // When Setting up expectations
        mock.expect_save().times(1).returning(|_| Ok(()));

        // Then The mock should work
        let chunk = Chunk::builder()
            .id(ChunkId::new(uuid::Uuid::new_v4()))
            .source(
                crate::knowledge::domain::ChunkSource::builder()
                    .file_path(ForestRelativePath::try_new("test.md").unwrap())
                    .repo_name(crate::knowledge::domain::RepoName::try_new("test-repo").unwrap())
                    .line_range(
                        crate::knowledge::domain::LineRange::builder()
                            .start(crate::knowledge::domain::LineNumber::try_new(1).unwrap())
                            .end(crate::knowledge::domain::LineNumber::try_new(5).unwrap())
                            .build(),
                    )
                    .build(),
            )
            .content(
                crate::knowledge::domain::ChunkContent::builder()
                    .text("test")
                    .token_count(crate::knowledge::domain::TokenCount::try_new(10).unwrap())
                    .build(),
            )
            .context(crate::knowledge::domain::ChunkContext::Markdown(
                crate::knowledge::domain::MarkdownContext::builder()
                    .heading_hierarchy(vec![])
                    .build(),
            ))
            .indexed_at(std::time::SystemTime::now())
            .build();

        assert!(mock.save(&chunk).is_ok());
    }
}
