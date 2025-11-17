//! Incremental index updates

use bon::Builder;
use snafu::{ResultExt, Snafu};
use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::{FileScanner, ScanError};
use crate::knowledge::chunking::{ChunkingDispatcher, ChunkingInput, DispatchError};
use crate::knowledge::domain::{
    Chunk, ChunkSource, ChunkableContent, Embedding, EmbeddingModelConfig, ForestRelativePath,
    IndexData, IndexMode, IndexedChunk, LineCount, LineNumber, LineRange, Timestamp,
};
use crate::knowledge::storage::{ChunkRepository, StorageError};

/// Manages incremental updates to the index
pub struct IncrementalUpdater<R: ChunkRepository> {
    scanner: FileScanner,
    dispatcher: ChunkingDispatcher,
    repository: R,
    mode: IndexMode,
}

impl<R: ChunkRepository> IncrementalUpdater<R> {
    /// Create a new incremental updater
    pub fn new(
        forest_root: impl AsRef<Path>,
        repository: R,
        config: &EmbeddingModelConfig,
        mode: IndexMode,
    ) -> Result<Self, UpdateError> {
        use update_error::*;

        let scanner = FileScanner::new(forest_root).context(ScanFailedSnafu)?;
        let dispatcher = ChunkingDispatcher::with_defaults(config).context(ChunkingFailedSnafu)?;

        Ok(Self {
            scanner,
            dispatcher,
            repository,
            mode,
        })
    }

    /// Update the index incrementally
    pub fn update(&mut self) -> Result<UpdateResult, UpdateError> {
        use update_error::*;

        let current_files = self.scanner.scan().context(ScanFailedSnafu)?;
        let indexed_chunks = self.repository.find_all().context(StorageFailedSnafu)?;

        let indexed_files: HashMap<ForestRelativePath, Timestamp> = indexed_chunks
            .iter()
            .map(|ic| (ic.chunk.source.file_path.clone(), ic.indexed_at))
            .fold(HashMap::new(), |mut map, (path, ts)| {
                map.entry(path).or_insert(ts);
                map
            });

        let current_paths: HashSet<_> = current_files
            .iter()
            .map(|f| f.relative_path.clone())
            .collect();
        let indexed_paths: HashSet<_> = indexed_files.keys().cloned().collect();

        let added: Vec<_> = current_files
            .iter()
            .filter(|f| !indexed_paths.contains(&f.relative_path))
            .collect();

        let modified: Vec<_> = current_files
            .iter()
            .filter(|f| {
                indexed_files
                    .get(&f.relative_path)
                    .map(|&indexed_ts| f.last_modified > indexed_ts)
                    .unwrap_or(false)
            })
            .collect();

        let deleted: Vec<_> = indexed_paths.difference(&current_paths).cloned().collect();

        let mut chunks_affected = 0;

        for path in &deleted {
            let removed = self
                .repository
                .delete_by_file(path)
                .context(StorageFailedSnafu)?;
            chunks_affected += removed;
        }

        for file in &added {
            let content = std::fs::read_to_string(file.absolute_path.to_string())
                .map_err(|e| ScanError::IoError {
                    source: e,
                    path: file.absolute_path.to_string(),
                })
                .context(ScanFailedSnafu)?;

            let input = ChunkingInput {
                content: ChunkableContent::new(content),
                source: ChunkSource::builder()
                    .file_path(file.relative_path.clone())
                    .repo_name(file.repo_name.clone())
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

                chunks_affected += indexed_chunks.len();
                self.repository
                    .save_batch(&indexed_chunks)
                    .context(StorageFailedSnafu)?;
            }
        }

        for file in &modified {
            let removed = self
                .repository
                .delete_by_file(&file.relative_path)
                .context(StorageFailedSnafu)?;
            chunks_affected += removed;

            let content = std::fs::read_to_string(file.absolute_path.to_string())
                .map_err(|e| ScanError::IoError {
                    source: e,
                    path: file.absolute_path.to_string(),
                })
                .context(ScanFailedSnafu)?;

            let input = ChunkingInput {
                content: ChunkableContent::new(content),
                source: ChunkSource::builder()
                    .file_path(file.relative_path.clone())
                    .repo_name(file.repo_name.clone())
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

                chunks_affected += indexed_chunks.len();
                self.repository
                    .save_batch(&indexed_chunks)
                    .context(StorageFailedSnafu)?;
            }
        }

        Ok(UpdateResult::builder()
            .files_added(added.len())
            .files_updated(modified.len())
            .files_removed(deleted.len())
            .chunks_affected(chunks_affected)
            .build())
    }

    fn index_chunk(&self, chunk: Chunk) -> Result<IndexedChunk, UpdateError> {
        let index_data = match self.mode {
            IndexMode::Fast => {
                let bm25_terms = crate::knowledge::storage::bm25::calculate_bm25_terms(
                    chunk.content.text.as_ref(),
                );
                IndexData::Fast { bm25_terms }
            }
            IndexMode::Best => {
                // TODO: Replace with real EmbeddingProvider when implemented (Phase 6, Commit 21-22)
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

/// Result of an incremental update operation
#[derive(Debug, Clone, PartialEq, Builder)]
#[non_exhaustive]
pub struct UpdateResult {
    pub files_added: usize,
    pub files_updated: usize,
    pub files_removed: usize,
    pub chunks_affected: usize,
}

/// Errors that can occur during incremental updates
#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum UpdateError {
    #[snafu(display("Failed to scan files"))]
    ScanFailed { source: ScanError },

    #[snafu(display("Failed to chunk file"))]
    ChunkingFailed { source: DispatchError },

    #[snafu(display("Failed to access storage"))]
    StorageFailed { source: StorageError },
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::knowledge::domain::{
        ChunkContent, ChunkContext, ChunkId, MarkdownContext, RepoName, TokenCount,
    };
    use crate::knowledge::storage::repository::MockChunkRepository;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_indexed_chunk(path: &str, timestamp: i64) -> IndexedChunk {
        IndexedChunk::builder()
            .chunk(
                Chunk::builder()
                    .id(ChunkId::new(uuid::Uuid::new_v4()))
                    .source(
                        ChunkSource::builder()
                            .file_path(ForestRelativePath::try_new(path).unwrap())
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
                            .text("test content")
                            .token_count(TokenCount::try_new(2).unwrap())
                            .build(),
                    )
                    .context(ChunkContext::Markdown(
                        MarkdownContext::builder().heading_hierarchy(vec![]).build(),
                    ))
                    .build(),
            )
            .index_data(IndexData::Fast {
                bm25_terms: std::collections::BTreeMap::new(),
            })
            .indexed_at(Timestamp::from_secs(timestamp))
            .build()
    }

    #[test]
    fn test_new_creates_updater_with_valid_forest() {
        // Given A valid forest directory
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();

        let mock_repo = MockChunkRepository::new();
        let config = EmbeddingModelConfig::default();

        // When Creating an IncrementalUpdater
        let result = IncrementalUpdater::new(temp_dir.path(), mock_repo, &config, IndexMode::Fast);

        // Then It should succeed
        assert!(result.is_ok());
    }

    #[test]
    fn test_new_fails_with_invalid_forest() {
        // Given A nonexistent forest directory
        let nonexistent = Path::new("/nonexistent/forest");
        let mock_repo = MockChunkRepository::new();
        let config = EmbeddingModelConfig::default();

        // When Creating an IncrementalUpdater
        let result = IncrementalUpdater::new(nonexistent, mock_repo, &config, IndexMode::Fast);

        // Then It should fail with ScanFailed error
        assert!(matches!(result, Err(UpdateError::ScanFailed { .. })));
    }

    #[test]
    fn test_update_adds_new_files() {
        // Given A forest with a new file and empty index
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("new.md"), "# New\n\nContent here").unwrap();

        let mut mock_repo = MockChunkRepository::new();
        mock_repo.expect_find_all().returning(|| Ok(vec![]));
        mock_repo.expect_save_batch().times(1).returning(|_| Ok(()));

        let config = EmbeddingModelConfig::default();
        let mut updater =
            IncrementalUpdater::new(temp_dir.path(), mock_repo, &config, IndexMode::Fast).unwrap();

        // When Updating the index
        let result = updater.update().unwrap();

        // Then It should report one file added
        assert_eq!(result.files_added, 1);
        assert_eq!(result.files_updated, 0);
        assert_eq!(result.files_removed, 0);
        assert!(result.chunks_affected > 0);
    }

    #[test]
    fn test_update_modifies_changed_files() {
        // Given A forest with a modified file
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("modified.md"), "# Modified\n\nNew content").unwrap();

        let indexed_chunk = create_test_indexed_chunk("test-repo/modified.md", 1000);

        let mut mock_repo = MockChunkRepository::new();
        mock_repo
            .expect_find_all()
            .returning(move || Ok(vec![indexed_chunk.clone()]));
        mock_repo
            .expect_delete_by_file()
            .times(1)
            .returning(|_| Ok(1));
        mock_repo.expect_save_batch().times(1).returning(|_| Ok(()));

        let config = EmbeddingModelConfig::default();
        let mut updater =
            IncrementalUpdater::new(temp_dir.path(), mock_repo, &config, IndexMode::Fast).unwrap();

        // When Updating the index
        let result = updater.update().unwrap();

        // Then It should report one file updated
        assert_eq!(result.files_added, 0);
        assert_eq!(result.files_updated, 1);
        assert_eq!(result.files_removed, 0);
        assert!(result.chunks_affected > 0);
    }

    #[test]
    fn test_update_removes_deleted_files() {
        // Given An index with a file that no longer exists
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();

        let indexed_chunk = create_test_indexed_chunk("test-repo/deleted.md", 1000);

        let mut mock_repo = MockChunkRepository::new();
        mock_repo
            .expect_find_all()
            .returning(move || Ok(vec![indexed_chunk.clone()]));
        mock_repo
            .expect_delete_by_file()
            .times(1)
            .returning(|_| Ok(2));

        let config = EmbeddingModelConfig::default();
        let mut updater =
            IncrementalUpdater::new(temp_dir.path(), mock_repo, &config, IndexMode::Fast).unwrap();

        // When Updating the index
        let result = updater.update().unwrap();

        // Then It should report one file removed
        assert_eq!(
            result,
            UpdateResult::builder()
                .files_added(0)
                .files_updated(0)
                .files_removed(1)
                .chunks_affected(2)
                .build()
        );
    }

    #[test]
    fn test_update_handles_mixed_changes() {
        // Given A forest with added, modified, and deleted files
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("new.md"), "# New\n\nContent").unwrap();
        fs::write(repo_dir.join("modified.md"), "# Modified\n\nContent").unwrap();

        let indexed_chunks = vec![
            create_test_indexed_chunk("test-repo/modified.md", 1000),
            create_test_indexed_chunk("test-repo/deleted.md", 1000),
        ];

        let mut mock_repo = MockChunkRepository::new();
        mock_repo
            .expect_find_all()
            .returning(move || Ok(indexed_chunks.clone()));
        mock_repo
            .expect_delete_by_file()
            .times(2)
            .returning(|_| Ok(1));
        mock_repo.expect_save_batch().times(2).returning(|_| Ok(()));

        let config = EmbeddingModelConfig::default();
        let mut updater =
            IncrementalUpdater::new(temp_dir.path(), mock_repo, &config, IndexMode::Fast).unwrap();

        // When Updating the index
        let result = updater.update().unwrap();

        // Then It should report all changes
        assert_eq!(result.files_added, 1);
        assert_eq!(result.files_updated, 1);
        assert_eq!(result.files_removed, 1);
        assert!(result.chunks_affected > 0);
    }

    #[test]
    fn test_update_propagates_storage_errors() {
        // Given A repository that fails to find chunks
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();

        let mut mock_repo = MockChunkRepository::new();
        mock_repo.expect_find_all().returning(|| {
            Err(StorageError::InvalidData {
                message: "test error".to_string(),
            })
        });

        let config = EmbeddingModelConfig::default();
        let mut updater =
            IncrementalUpdater::new(temp_dir.path(), mock_repo, &config, IndexMode::Fast).unwrap();

        // When Updating the index
        let result = updater.update();

        // Then It should fail with StorageFailed error
        assert!(matches!(result, Err(UpdateError::StorageFailed { .. })));
    }

    #[test]
    fn test_update_handles_empty_forest() {
        // Given An empty forest with no indexed files
        let temp_dir = TempDir::new().unwrap();

        let mut mock_repo = MockChunkRepository::new();
        mock_repo.expect_find_all().returning(|| Ok(vec![]));

        let config = EmbeddingModelConfig::default();
        let mut updater =
            IncrementalUpdater::new(temp_dir.path(), mock_repo, &config, IndexMode::Fast).unwrap();

        // When Updating the index
        let result = updater.update().unwrap();

        // Then It should report no changes
        assert_eq!(
            result,
            UpdateResult::builder()
                .files_added(0)
                .files_updated(0)
                .files_removed(0)
                .chunks_affected(0)
                .build()
        );
    }
}
