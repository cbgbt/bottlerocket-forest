//! Index building orchestration

use bon::Builder;
use snafu::{ResultExt, Snafu};
use std::path::Path;
use std::time::Duration;

use super::{FileScanner, ScanError};
use crate::knowledge::chunking::{ChunkingDispatcher, ChunkingInput, DispatchError};
use crate::knowledge::domain::{
    Chunk, ChunkSource, ChunkableContent, Embedding, EmbeddingModelConfig, IndexData, IndexMode,
    IndexedChunk, LineCount, LineNumber, LineRange, Timestamp,
};
use crate::knowledge::storage::{ChunkRepository, StorageError};

/// Orchestrates the index building process
pub struct IndexBuilder<R: ChunkRepository> {
    scanner: FileScanner,
    dispatcher: ChunkingDispatcher,
    repository: R,
    mode: IndexMode,
}

impl<R: ChunkRepository> IndexBuilder<R> {
    /// Create a new index builder
    pub fn new(
        forest_root: impl AsRef<Path>,
        repository: R,
        config: &EmbeddingModelConfig,
        mode: IndexMode,
    ) -> Result<Self, IndexBuildError> {
        use index_build_error::*;

        let scanner = FileScanner::new(forest_root).context(ScanFailedSnafu)?;
        let dispatcher = ChunkingDispatcher::with_defaults(config).context(ChunkingFailedSnafu)?;

        Ok(Self {
            scanner,
            dispatcher,
            repository,
            mode,
        })
    }

    /// Build the index from scratch
    pub fn build(&mut self) -> Result<IndexBuildResult, IndexBuildError> {
        use index_build_error::*;

        let start = std::time::Instant::now();
        let mut files_indexed = 0;
        let mut chunks_created = 0;

        let files = self.scanner.scan().context(ScanFailedSnafu)?;

        for file in files {
            let content = std::fs::read_to_string(file.absolute_path.to_string())
                .map_err(|e| ScanError::IoError {
                    source: e,
                    path: file.absolute_path.to_string(),
                })
                .context(ScanFailedSnafu)?;

            let input = ChunkingInput {
                content: ChunkableContent::new(content),
                source: ChunkSource::builder()
                    .file_path(file.relative_path)
                    .repo_name(file.repo_name)
                    .line_range(
                        LineRange::builder()
                            .start(LineNumber::try_new(1).expect("1 is valid line number"))
                            .line_count(LineCount::try_new(1).expect("1 is valid line count"))
                            .build(),
                    )
                    .build(),
            };

            if let Some(result) = self.dispatcher.chunk_file(&input) {
                let chunks = result.context(ChunkingFailedSnafu)?;

                let indexed_chunks: Vec<IndexedChunk> = chunks
                    .into_iter()
                    .map(|chunk| self.index_chunk(chunk))
                    .collect::<Result<Vec<_>, _>>()?;

                chunks_created += indexed_chunks.len();
                self.repository
                    .save_batch(&indexed_chunks)
                    .context(StorageFailedSnafu)?;
                files_indexed += 1;
            }
        }

        Ok(IndexBuildResult::builder()
            .files_indexed(files_indexed)
            .chunks_created(chunks_created)
            .duration(start.elapsed())
            .mode(self.mode)
            .build())
    }

    /// Rebuild the index (clear then build)
    pub fn rebuild(&mut self) -> Result<IndexBuildResult, IndexBuildError> {
        use index_build_error::*;

        self.repository.clear().context(StorageFailedSnafu)?;
        self.build()
    }

    fn index_chunk(&self, chunk: Chunk) -> Result<IndexedChunk, IndexBuildError> {
        let index_data = match self.mode {
            IndexMode::Fast => {
                let bm25_terms = crate::knowledge::storage::bm25::calculate_bm25_terms(
                    chunk.content.text.as_ref(),
                );
                IndexData::Fast { bm25_terms }
            }
            IndexMode::Best => {
                // TODO: Replace with real EmbeddingProvider when implemented (Phase 6, Commit 21-22)
                // For now, create a placeholder embedding with the correct dimensions
                let embedding = Embedding::try_new(vec![0.0; 384])
                    .expect("placeholder embedding should be valid");
                IndexData::Best { embedding }
            }
        };

        Ok(IndexedChunk::builder()
            .chunk(chunk)
            .index_data(index_data)
            .indexed_at(Timestamp::now())
            .build())
    }
}

/// Result of an index build operation
#[derive(Debug, Clone, PartialEq, Builder)]
#[non_exhaustive]
pub struct IndexBuildResult {
    pub files_indexed: usize,
    pub chunks_created: usize,
    pub duration: Duration,
    pub mode: IndexMode,
}

/// Errors that can occur during index building
#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum IndexBuildError {
    #[snafu(display("Failed to scan files"))]
    ScanFailed { source: ScanError },

    #[snafu(display("Failed to chunk file"))]
    ChunkingFailed { source: DispatchError },

    #[snafu(display("Failed to store chunks"))]
    StorageFailed { source: StorageError },
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::knowledge::domain::{
        ChunkContent, ChunkContext, ChunkId, ChunkSource, ForestRelativePath, LineCount,
        LineNumber, LineRange, MarkdownContext, RepoName, TokenCount,
    };
    use crate::knowledge::storage::repository::MockChunkRepository;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_chunk(file_path: &str, content: &str) -> Chunk {
        Chunk::builder()
            .id(ChunkId::new(uuid::Uuid::new_v4()))
            .source(
                ChunkSource::builder()
                    .file_path(ForestRelativePath::try_new(file_path).unwrap())
                    .repo_name(RepoName::try_new("test-repo").unwrap())
                    .line_range(
                        LineRange::builder()
                            .start(LineNumber::try_new(1).unwrap())
                            .line_count(LineCount::try_new(1).unwrap())
                            .build(),
                    )
                    .build(),
            )
            .content(
                ChunkContent::builder()
                    .text(content.to_string())
                    .token_count(TokenCount::try_new(content.split_whitespace().count()).unwrap())
                    .build(),
            )
            .context(ChunkContext::Markdown(
                MarkdownContext::builder().heading_hierarchy(vec![]).build(),
            ))
            .build()
    }

    #[test]
    fn test_new_creates_builder_with_valid_forest() {
        // Given A valid forest directory
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();

        let mock_repo = MockChunkRepository::new();
        let config = EmbeddingModelConfig::default();

        // When Creating an IndexBuilder
        let result = IndexBuilder::new(temp_dir.path(), mock_repo, &config, IndexMode::Fast);

        // Then It should succeed
        assert!(result.is_ok());
    }

    #[test]
    fn test_new_fails_with_invalid_forest() {
        // Given A nonexistent forest directory
        let nonexistent = Path::new("/nonexistent/forest");
        let mock_repo = MockChunkRepository::new();
        let config = EmbeddingModelConfig::default();

        // When Creating an IndexBuilder
        let result = IndexBuilder::new(nonexistent, mock_repo, &config, IndexMode::Fast);

        // Then It should fail with ScanFailed error
        assert!(matches!(result, Err(IndexBuildError::ScanFailed { .. })));
    }

    #[test]
    fn test_build_indexes_markdown_files() {
        // Given A forest with markdown files
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("README.md"), "# Test\nContent here").unwrap();

        let mut mock_repo = MockChunkRepository::new();
        mock_repo.expect_save_batch().times(1).returning(|chunks| {
            assert!(!chunks.is_empty());
            Ok(())
        });

        let config = EmbeddingModelConfig::default();
        let mut builder =
            IndexBuilder::new(temp_dir.path(), mock_repo, &config, IndexMode::Fast).unwrap();

        // When Building the index
        let result = builder.build();

        // Then It should succeed and report indexed files
        assert!(result.is_ok());
        let build_result = result.unwrap();
        assert_eq!(build_result.files_indexed, 1);
        assert!(build_result.chunks_created > 0);
        assert_eq!(build_result.mode, IndexMode::Fast);
    }

    #[test]
    fn test_build_indexes_rust_files() {
        // Given A forest with rust files
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir_all(repo_dir.join("src")).unwrap();
        fs::write(
            repo_dir.join("src/lib.rs"),
            "/// Documentation\npub fn test() {}",
        )
        .unwrap();

        let mut mock_repo = MockChunkRepository::new();
        mock_repo.expect_save_batch().times(1).returning(|chunks| {
            assert!(!chunks.is_empty());
            Ok(())
        });

        let config = EmbeddingModelConfig::default();
        let mut builder =
            IndexBuilder::new(temp_dir.path(), mock_repo, &config, IndexMode::Fast).unwrap();

        // When Building the index
        let result = builder.build();

        // Then It should succeed and report indexed files
        assert!(result.is_ok());
        let build_result = result.unwrap();
        assert_eq!(build_result.files_indexed, 1);
        assert!(build_result.chunks_created > 0);
    }

    #[test]
    fn test_build_wraps_chunks_with_fast_mode_data() {
        // Given A forest with a markdown file in Fast mode
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(
            repo_dir.join("test.md"),
            "# Rust programming\n\nRust is a systems programming language.",
        )
        .unwrap();

        let mut mock_repo = MockChunkRepository::new();
        mock_repo.expect_save_batch().times(1).returning(|chunks| {
            assert_eq!(chunks.len(), 1);
            assert!(matches!(chunks[0].index_data, IndexData::Fast { .. }));
            if let IndexData::Fast { ref bm25_terms } = chunks[0].index_data {
                assert!(bm25_terms.contains_key("rust"));
                assert!(bm25_terms.contains_key("programming"));
            }
            Ok(())
        });

        let config = EmbeddingModelConfig::default();
        let mut builder =
            IndexBuilder::new(temp_dir.path(), mock_repo, &config, IndexMode::Fast).unwrap();

        // When Building the index
        let result = builder.build();

        // Then Chunks should have BM25 terms
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_wraps_chunks_with_best_mode_data() {
        // Given A forest with a markdown file in Best mode
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(
            repo_dir.join("test.md"),
            "# Test content\n\nThis is test content for indexing.",
        )
        .unwrap();

        let mut mock_repo = MockChunkRepository::new();
        mock_repo.expect_save_batch().times(1).returning(|chunks| {
            assert_eq!(chunks.len(), 1);
            assert!(matches!(chunks[0].index_data, IndexData::Best { .. }));
            Ok(())
        });

        let config = EmbeddingModelConfig::default();
        let mut builder =
            IndexBuilder::new(temp_dir.path(), mock_repo, &config, IndexMode::Best).unwrap();

        // When Building the index
        let result = builder.build();

        // Then Chunks should have embeddings
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_propagates_storage_errors() {
        // Given A repository that fails to store
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("test.md"), "# Test").unwrap();

        let mut mock_repo = MockChunkRepository::new();
        mock_repo.expect_save_batch().returning(|_| {
            Err(StorageError::InvalidData {
                message: "test error".to_string(),
            })
        });

        let config = EmbeddingModelConfig::default();
        let mut builder =
            IndexBuilder::new(temp_dir.path(), mock_repo, &config, IndexMode::Fast).unwrap();

        // When Building the index
        let result = builder.build();

        // Then It should fail with StorageFailed error
        assert!(matches!(result, Err(IndexBuildError::StorageFailed { .. })));
    }

    #[test]
    fn test_rebuild_clears_then_builds() {
        // Given A forest with files
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("test.md"), "# Test").unwrap();

        let mut mock_repo = MockChunkRepository::new();
        mock_repo.expect_clear().times(1).returning(|| Ok(5));
        mock_repo.expect_save_batch().times(1).returning(|_| Ok(()));

        let config = EmbeddingModelConfig::default();
        let mut builder =
            IndexBuilder::new(temp_dir.path(), mock_repo, &config, IndexMode::Fast).unwrap();

        // When Rebuilding the index
        let result = builder.rebuild();

        // Then It should clear then build
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_handles_empty_forest() {
        // Given An empty forest directory
        let temp_dir = TempDir::new().unwrap();

        let mut mock_repo = MockChunkRepository::new();
        mock_repo.expect_save_batch().times(0);

        let config = EmbeddingModelConfig::default();
        let mut builder =
            IndexBuilder::new(temp_dir.path(), mock_repo, &config, IndexMode::Fast).unwrap();

        // When Building the index
        let result = builder.build().unwrap();

        // Then It should succeed with zero files indexed
        assert_eq!(
            result,
            IndexBuildResult::builder()
                .files_indexed(0)
                .chunks_created(0)
                .duration(result.duration)
                .mode(IndexMode::Fast)
                .build()
        );
    }

    #[test]
    fn test_build_counts_chunks_correctly() {
        // Given A forest with multiple files
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("file1.md"), "# Test 1\nContent").unwrap();
        fs::write(repo_dir.join("file2.md"), "# Test 2\nMore content").unwrap();

        let mut mock_repo = MockChunkRepository::new();
        mock_repo
            .expect_save_batch()
            .times(2)
            .returning(|_chunks| Ok(()));

        let config = EmbeddingModelConfig::default();
        let mut builder =
            IndexBuilder::new(temp_dir.path(), mock_repo, &config, IndexMode::Fast).unwrap();

        // When Building the index
        let result = builder.build().unwrap();

        // Then Chunk count should reflect all chunks from all files
        assert_eq!(result.files_indexed, 2);
        assert!(result.chunks_created >= 2);
    }

    #[test]
    fn test_index_chunk_adds_timestamp() {
        // Given A chunk and a builder
        let temp_dir = TempDir::new().unwrap();
        fs::create_dir(temp_dir.path().join("test-repo")).unwrap();

        let mock_repo = MockChunkRepository::new();
        let config = EmbeddingModelConfig::default();
        let builder =
            IndexBuilder::new(temp_dir.path(), mock_repo, &config, IndexMode::Fast).unwrap();

        let chunk = create_test_chunk("test.md", "test content");
        let before = Timestamp::now();

        // When Indexing the chunk
        let indexed = builder.index_chunk(chunk).unwrap();

        // Then It should have a timestamp
        let after = Timestamp::now();
        assert!(indexed.indexed_at >= before);
        assert!(indexed.indexed_at <= after);
    }
}
