//! Unified indexer for building and updating the knowledge index

mod operations;
mod types;

pub use types::{IndexError, IndexResult};

use snafu::ResultExt;
use std::collections::HashSet;
use std::path::Path;
use std::time::Instant;

use crate::knowledge::chunking::ChunkingDispatcher;
use crate::knowledge::domain::{EmbeddingModelConfig, IndexMode};
use crate::knowledge::storage::ChunkRepository;

use super::FileScanner;

/// Strategy for index operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexStrategy {
    /// Build index from scratch (don't clear existing)
    Build,

    /// Clear existing index then build from scratch
    Rebuild,

    /// Update only changed files
    Incremental,
}

/// Unified indexer for building and updating the knowledge index
pub struct Indexer<R: ChunkRepository> {
    scanner: FileScanner,
    dispatcher: ChunkingDispatcher,
    repository: R,
    mode: IndexMode,
}

impl<R: ChunkRepository> Indexer<R> {
    /// Create a new indexer
    pub fn new(
        forest_root: impl AsRef<Path>,
        repository: R,
        config: &EmbeddingModelConfig,
        mode: IndexMode,
    ) -> Result<Self, IndexError> {
        use types::index_error::*;

        let scanner = FileScanner::new(forest_root).context(ScanFailedSnafu)?;
        let dispatcher = ChunkingDispatcher::with_defaults(config).context(ChunkingFailedSnafu)?;

        Ok(Self {
            scanner,
            dispatcher,
            repository,
            mode,
        })
    }

    /// Execute an indexing operation
    pub fn index(&mut self, strategy: IndexStrategy) -> Result<IndexResult, IndexError> {
        match strategy {
            IndexStrategy::Build => self.build(),
            IndexStrategy::Rebuild => self.rebuild(),
            IndexStrategy::Incremental => self.incremental(),
        }
    }

    /// Build index from scratch (don't clear existing)
    fn build(&mut self) -> Result<IndexResult, IndexError> {
        use types::index_error::*;

        let start = Instant::now();
        let mut files_added = 0;
        let mut chunks_affected = 0;

        let files = self.scanner.scan().context(ScanFailedSnafu)?;

        for file in files {
            let indexed_chunks = operations::process_file(&file, &self.dispatcher, self.mode)?;

            if !indexed_chunks.is_empty() {
                chunks_affected += indexed_chunks.len();
                self.repository
                    .save_batch(&indexed_chunks)
                    .context(StorageFailedSnafu)?;
                files_added += 1;
            }
        }

        Ok(IndexResult::builder()
            .files_processed(files_added)
            .files_added(files_added)
            .files_updated(0)
            .files_removed(0)
            .chunks_affected(chunks_affected)
            .duration(start.elapsed())
            .mode(self.mode)
            .build())
    }

    /// Clear existing index then build from scratch
    fn rebuild(&mut self) -> Result<IndexResult, IndexError> {
        use types::index_error::*;

        self.repository.clear().context(StorageFailedSnafu)?;
        self.build()
    }

    /// Update only changed files
    fn incremental(&mut self) -> Result<IndexResult, IndexError> {
        use types::index_error::*;

        let current_files = self.scanner.scan().context(ScanFailedSnafu)?;
        let indexed_files = self
            .repository
            .get_indexed_files()
            .context(StorageFailedSnafu)?;

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

        for file in added.iter().chain(modified.iter()) {
            if modified
                .iter()
                .any(|m| m.relative_path == file.relative_path)
            {
                let removed = self
                    .repository
                    .delete_by_file(&file.relative_path)
                    .context(StorageFailedSnafu)?;
                chunks_affected += removed;
            }

            let indexed_chunks = operations::process_file(file, &self.dispatcher, self.mode)?;

            if !indexed_chunks.is_empty() {
                chunks_affected += indexed_chunks.len();
                self.repository
                    .save_batch(&indexed_chunks)
                    .context(StorageFailedSnafu)?;
            }
        }

        Ok(IndexResult::builder()
            .files_processed(added.len() + modified.len())
            .files_added(added.len())
            .files_updated(modified.len())
            .files_removed(deleted.len())
            .chunks_affected(chunks_affected)
            .duration(std::time::Duration::from_secs(0))
            .mode(self.mode)
            .build())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::knowledge::domain::{ForestRelativePath, Timestamp};
    use crate::knowledge::storage::StorageError;
    use crate::knowledge::storage::repository::MockChunkRepository;
    use std::collections::HashMap;
    use std::fs;
    use tempfile::TempDir;

    // Constructor tests
    #[test]
    fn test_new_creates_indexer_with_valid_forest() {
        // Given A valid forest directory
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();

        let mock_repo = MockChunkRepository::new();
        let config = EmbeddingModelConfig::default();

        // When Creating an Indexer
        let result = Indexer::new(temp_dir.path(), mock_repo, &config, IndexMode::Fast);

        // Then It should succeed
        assert!(result.is_ok());
    }

    #[test]
    fn test_new_fails_with_invalid_forest() {
        // Given A nonexistent forest directory
        let nonexistent = Path::new("/nonexistent/forest");
        let mock_repo = MockChunkRepository::new();
        let config = EmbeddingModelConfig::default();

        // When Creating an Indexer
        let result = Indexer::new(nonexistent, mock_repo, &config, IndexMode::Fast);

        // Then It should fail with ScanFailed error
        assert!(matches!(result, Err(IndexError::ScanFailed { .. })));
    }

    // Build strategy tests
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
        let mut indexer =
            Indexer::new(temp_dir.path(), mock_repo, &config, IndexMode::Fast).unwrap();

        // When Building the index
        let result = indexer.index(IndexStrategy::Build);

        // Then It should succeed and report indexed files
        assert!(result.is_ok());
        let index_result = result.unwrap();
        assert_eq!(index_result.files_added, 1);
        assert_eq!(index_result.files_processed, 1);
        assert!(index_result.chunks_affected > 0);
        assert_eq!(index_result.mode, IndexMode::Fast);
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
        let mut indexer =
            Indexer::new(temp_dir.path(), mock_repo, &config, IndexMode::Fast).unwrap();

        // When Building the index
        let result = indexer.index(IndexStrategy::Build);

        // Then It should succeed and report indexed files
        assert!(result.is_ok());
        let index_result = result.unwrap();
        assert_eq!(index_result.files_added, 1);
        assert!(index_result.chunks_affected > 0);
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
            assert!(matches!(
                chunks[0].index_data,
                crate::knowledge::domain::IndexData::Fast { .. }
            ));
            if let crate::knowledge::domain::IndexData::Fast { ref bm25_terms } =
                chunks[0].index_data
            {
                assert!(bm25_terms.contains_key("rust"));
                assert!(bm25_terms.contains_key("programming"));
            }
            Ok(())
        });

        let config = EmbeddingModelConfig::default();
        let mut indexer =
            Indexer::new(temp_dir.path(), mock_repo, &config, IndexMode::Fast).unwrap();

        // When Building the index
        let result = indexer.index(IndexStrategy::Build);

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
            assert!(matches!(
                chunks[0].index_data,
                crate::knowledge::domain::IndexData::Best { .. }
            ));
            Ok(())
        });

        let config = EmbeddingModelConfig::default();
        let mut indexer =
            Indexer::new(temp_dir.path(), mock_repo, &config, IndexMode::Best).unwrap();

        // When Building the index
        let result = indexer.index(IndexStrategy::Build);

        // Then Chunks should have embeddings
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_propagates_storage_errors() {
        // Given A repository that fails to store
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("test.md"), "# Test\n\nSome content here").unwrap();

        let mut mock_repo = MockChunkRepository::new();
        mock_repo.expect_save_batch().returning(|_| {
            Err(StorageError::InvalidData {
                message: "test error".to_string(),
            })
        });

        let config = EmbeddingModelConfig::default();
        let mut indexer =
            Indexer::new(temp_dir.path(), mock_repo, &config, IndexMode::Fast).unwrap();

        // When Building the index
        let result = indexer.index(IndexStrategy::Build);

        // Then It should fail with StorageFailed error
        assert!(matches!(result, Err(IndexError::StorageFailed { .. })));
    }

    #[test]
    fn test_build_handles_empty_forest() {
        // Given An empty forest directory
        let temp_dir = TempDir::new().unwrap();

        let mut mock_repo = MockChunkRepository::new();
        mock_repo.expect_save_batch().times(0);

        let config = EmbeddingModelConfig::default();
        let mut indexer =
            Indexer::new(temp_dir.path(), mock_repo, &config, IndexMode::Fast).unwrap();

        // When Building the index
        let result = indexer.index(IndexStrategy::Build).unwrap();

        // Then It should succeed with zero files indexed
        assert_eq!(result.files_added, 0);
        assert_eq!(result.files_processed, 0);
        assert_eq!(result.chunks_affected, 0);
        assert_eq!(result.mode, IndexMode::Fast);
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
        let mut indexer =
            Indexer::new(temp_dir.path(), mock_repo, &config, IndexMode::Fast).unwrap();

        // When Building the index
        let result = indexer.index(IndexStrategy::Build).unwrap();

        // Then Chunk count should reflect all chunks from all files
        assert_eq!(result.files_added, 2);
        assert_eq!(result.files_processed, 2);
        assert!(result.chunks_affected >= 2);
    }

    // Rebuild strategy tests
    #[test]
    fn test_rebuild_clears_then_builds() {
        // Given A forest with files
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("test.md"), "# Test\n\nContent here").unwrap();

        let mut mock_repo = MockChunkRepository::new();
        mock_repo.expect_clear().times(1).returning(|| Ok(5));
        mock_repo.expect_save_batch().times(1).returning(|_| Ok(()));

        let config = EmbeddingModelConfig::default();
        let mut indexer =
            Indexer::new(temp_dir.path(), mock_repo, &config, IndexMode::Fast).unwrap();

        // When Rebuilding the index
        let result = indexer.index(IndexStrategy::Rebuild);

        // Then It should clear then build
        assert!(result.is_ok());
    }

    // Incremental strategy tests
    #[test]
    fn test_incremental_adds_new_files() {
        // Given A forest with a new file and empty index
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("new.md"), "# New\n\nContent here").unwrap();

        let mut mock_repo = MockChunkRepository::new();
        mock_repo
            .expect_get_indexed_files()
            .returning(|| Ok(HashMap::new()));
        mock_repo.expect_save_batch().times(1).returning(|_| Ok(()));

        let config = EmbeddingModelConfig::default();
        let mut indexer =
            Indexer::new(temp_dir.path(), mock_repo, &config, IndexMode::Fast).unwrap();

        // When Updating the index
        let result = indexer.index(IndexStrategy::Incremental).unwrap();

        // Then It should report one file added
        assert_eq!(result.files_added, 1);
        assert_eq!(result.files_updated, 0);
        assert_eq!(result.files_removed, 0);
        assert_eq!(result.files_processed, 1);
        assert!(result.chunks_affected > 0);
    }

    #[test]
    fn test_incremental_modifies_changed_files() {
        // Given A forest with a modified file
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("modified.md"), "# Modified\n\nNew content").unwrap();

        let mut mock_repo = MockChunkRepository::new();
        mock_repo.expect_get_indexed_files().returning(|| {
            let mut map = HashMap::new();
            map.insert(
                ForestRelativePath::try_new("test-repo/modified.md").unwrap(),
                Timestamp::from_secs(1000),
            );
            Ok(map)
        });
        mock_repo
            .expect_delete_by_file()
            .times(1)
            .returning(|_| Ok(1));
        mock_repo.expect_save_batch().times(1).returning(|_| Ok(()));

        let config = EmbeddingModelConfig::default();
        let mut indexer =
            Indexer::new(temp_dir.path(), mock_repo, &config, IndexMode::Fast).unwrap();

        // When Updating the index
        let result = indexer.index(IndexStrategy::Incremental).unwrap();

        // Then It should report one file updated
        assert_eq!(result.files_added, 0);
        assert_eq!(result.files_updated, 1);
        assert_eq!(result.files_removed, 0);
        assert_eq!(result.files_processed, 1);
        assert!(result.chunks_affected > 0);
    }

    #[test]
    fn test_incremental_removes_deleted_files() {
        // Given An index with a file that no longer exists
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();

        let mut mock_repo = MockChunkRepository::new();
        mock_repo.expect_get_indexed_files().returning(|| {
            let mut map = HashMap::new();
            map.insert(
                ForestRelativePath::try_new("test-repo/deleted.md").unwrap(),
                Timestamp::from_secs(1000),
            );
            Ok(map)
        });
        mock_repo
            .expect_delete_by_file()
            .times(1)
            .returning(|_| Ok(2));

        let config = EmbeddingModelConfig::default();
        let mut indexer =
            Indexer::new(temp_dir.path(), mock_repo, &config, IndexMode::Fast).unwrap();

        // When Updating the index
        let result = indexer.index(IndexStrategy::Incremental).unwrap();

        // Then It should report one file removed
        assert_eq!(result.files_added, 0);
        assert_eq!(result.files_updated, 0);
        assert_eq!(result.files_removed, 1);
        assert_eq!(result.files_processed, 0);
        assert_eq!(result.chunks_affected, 2);
    }

    #[test]
    fn test_incremental_handles_mixed_changes() {
        // Given A forest with added, modified, and deleted files
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("new.md"), "# New\n\nContent").unwrap();
        fs::write(repo_dir.join("modified.md"), "# Modified\n\nContent").unwrap();

        let mut mock_repo = MockChunkRepository::new();
        mock_repo.expect_get_indexed_files().returning(|| {
            let mut map = HashMap::new();
            map.insert(
                ForestRelativePath::try_new("test-repo/modified.md").unwrap(),
                Timestamp::from_secs(1000),
            );
            map.insert(
                ForestRelativePath::try_new("test-repo/deleted.md").unwrap(),
                Timestamp::from_secs(1000),
            );
            Ok(map)
        });
        mock_repo
            .expect_delete_by_file()
            .times(2)
            .returning(|_| Ok(1));
        mock_repo.expect_save_batch().times(2).returning(|_| Ok(()));

        let config = EmbeddingModelConfig::default();
        let mut indexer =
            Indexer::new(temp_dir.path(), mock_repo, &config, IndexMode::Fast).unwrap();

        // When Updating the index
        let result = indexer.index(IndexStrategy::Incremental).unwrap();

        // Then It should report all changes
        assert_eq!(result.files_added, 1);
        assert_eq!(result.files_updated, 1);
        assert_eq!(result.files_removed, 1);
        assert_eq!(result.files_processed, 2);
        assert!(result.chunks_affected > 0);
    }

    #[test]
    fn test_incremental_propagates_storage_errors() {
        // Given A repository that fails to get indexed files
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();

        let mut mock_repo = MockChunkRepository::new();
        mock_repo.expect_get_indexed_files().returning(|| {
            Err(StorageError::InvalidData {
                message: "test error".to_string(),
            })
        });

        let config = EmbeddingModelConfig::default();
        let mut indexer =
            Indexer::new(temp_dir.path(), mock_repo, &config, IndexMode::Fast).unwrap();

        // When Updating the index
        let result = indexer.index(IndexStrategy::Incremental);

        // Then It should fail with StorageFailed error
        assert!(matches!(result, Err(IndexError::StorageFailed { .. })));
    }

    #[test]
    fn test_incremental_handles_empty_forest() {
        // Given An empty forest with no indexed files
        let temp_dir = TempDir::new().unwrap();

        let mut mock_repo = MockChunkRepository::new();
        mock_repo
            .expect_get_indexed_files()
            .returning(|| Ok(HashMap::new()));

        let config = EmbeddingModelConfig::default();
        let mut indexer =
            Indexer::new(temp_dir.path(), mock_repo, &config, IndexMode::Fast).unwrap();

        // When Updating the index
        let result = indexer.index(IndexStrategy::Incremental).unwrap();

        // Then It should report no changes
        assert_eq!(result.files_added, 0);
        assert_eq!(result.files_updated, 0);
        assert_eq!(result.files_removed, 0);
        assert_eq!(result.files_processed, 0);
        assert_eq!(result.chunks_affected, 0);
    }
}
