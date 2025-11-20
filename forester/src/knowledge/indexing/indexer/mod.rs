//! Unified indexer for building and updating the knowledge index

mod operations;
mod types;

pub use types::{IndexResult, IndexingError};

use snafu::ResultExt;
use std::collections::HashSet;
use std::path::Path;
use std::time::Instant;

use crate::knowledge::chunking::ChunkingDispatcher;
use crate::knowledge::domain::{EmbeddingModelConfig, IndexMode, ScanConfig};
use crate::knowledge::storage::ChunkRepository;

use super::{FileScanner, IndexDataProvider};

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
    provider: Box<dyn IndexDataProvider>,
}

impl<R: ChunkRepository> Indexer<R> {
    /// Create a new indexer
    pub fn new(
        forest_root: impl AsRef<Path>,
        repository: R,
        config: &EmbeddingModelConfig,
        provider: Box<dyn IndexDataProvider>,
        scan_config: ScanConfig,
    ) -> Result<Self, IndexingError> {
        use types::indexing_error::*;

        let scanner =
            FileScanner::with_config(forest_root, scan_config).context(ScanFailedSnafu)?;
        let dispatcher = ChunkingDispatcher::with_defaults(config).context(ChunkingFailedSnafu)?;

        Ok(Self {
            scanner,
            dispatcher,
            repository,
            provider,
        })
    }

    /// Execute an indexing operation
    pub fn index(&mut self, strategy: IndexStrategy) -> Result<IndexResult, IndexingError> {
        match strategy {
            IndexStrategy::Build => self.build(),
            IndexStrategy::Rebuild => self.rebuild(),
            IndexStrategy::Incremental => self.incremental(),
        }
    }

    /// Build index from scratch (don't clear existing)
    fn build(&mut self) -> Result<IndexResult, IndexingError> {
        use rayon::prelude::*;
        use types::indexing_error::*;

        let start = Instant::now();

        let files = self.scanner.scan().context(ScanFailedSnafu)?;

        let results: Vec<_> = files
            .par_iter()
            .map(|file| {
                operations::process_file_gracefully(file, &self.dispatcher, &*self.provider)
            })
            .collect();

        let mut files_added = 0;
        let mut files_skipped = 0;
        let mut chunks_affected = 0;

        for result in results {
            match result {
                Ok(indexed_chunks) => {
                    if !indexed_chunks.is_empty() {
                        chunks_affected += indexed_chunks.len();
                        self.repository
                            .save_batch(&indexed_chunks)
                            .context(StorageFailedSnafu)?;
                        files_added += 1;
                    }
                }
                Err(Ok(())) => {
                    files_skipped += 1;
                }
                Err(Err(e)) => {
                    return Err(e);
                }
            }
        }

        Ok(IndexResult::builder()
            .files_processed(files_added)
            .files_added(files_added)
            .files_updated(0)
            .files_removed(0)
            .files_skipped(files_skipped)
            .chunks_affected(chunks_affected)
            .duration(start.elapsed())
            .mode(IndexMode::Best)
            .build())
    }

    /// Clear existing index then build from scratch
    fn rebuild(&mut self) -> Result<IndexResult, IndexingError> {
        use types::indexing_error::*;

        self.repository.clear().context(StorageFailedSnafu)?;
        self.build()
    }

    /// Update only changed files
    fn incremental(&mut self) -> Result<IndexResult, IndexingError> {
        use rayon::prelude::*;
        use types::indexing_error::*;

        let start = Instant::now();

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

        let files_to_process: Vec<_> = added.iter().chain(modified.iter()).copied().collect();

        let results: Vec<_> = files_to_process
            .par_iter()
            .map(|file| {
                operations::process_file_gracefully(file, &self.dispatcher, &*self.provider)
            })
            .collect();

        let mut files_skipped = 0;
        let mut fatal_error = None;

        for (file, result) in files_to_process.iter().zip(results.into_iter()) {
            match result {
                Ok(indexed_chunks) => {
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

                    if !indexed_chunks.is_empty() {
                        chunks_affected += indexed_chunks.len();
                        self.repository
                            .save_batch(&indexed_chunks)
                            .context(StorageFailedSnafu)?;
                    }
                }
                Err(Ok(())) => {
                    files_skipped += 1;
                }
                Err(Err(e)) => {
                    fatal_error = Some(e);
                    break;
                }
            }
        }

        if let Some(e) = fatal_error {
            return Err(e);
        }

        Ok(IndexResult::builder()
            .files_processed(added.len() + modified.len() - files_skipped)
            .files_added(added.len())
            .files_updated(modified.len())
            .files_removed(deleted.len())
            .files_skipped(files_skipped)
            .chunks_affected(chunks_affected)
            .duration(start.elapsed())
            .mode(IndexMode::Best)
            .build())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::knowledge::domain::{Embedding, ForestRelativePath, IndexMode, Timestamp};
    use crate::knowledge::indexing::provider::MockIndexDataProvider;
    use crate::knowledge::storage::StorageError;
    use crate::knowledge::storage::repository::MockChunkRepository;
    use std::collections::HashMap;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_new_creates_indexer_with_valid_forest() {
        // Given A valid forest directory and mock provider
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();

        let mock_repo = MockChunkRepository::new();
        let config = EmbeddingModelConfig::default();
        let mut mock_provider = MockIndexDataProvider::new();

        // When Creating an Indexer
        let result = Indexer::new(
            temp_dir.path(),
            mock_repo,
            &config,
            Box::new(mock_provider),
            ScanConfig::default(),
        );

        // Then It should succeed
        assert!(result.is_ok());
    }

    #[test]
    fn test_new_fails_with_invalid_forest() {
        // Given A nonexistent forest directory
        let nonexistent = Path::new("/nonexistent/forest");
        let mock_repo = MockChunkRepository::new();
        let config = EmbeddingModelConfig::default();
        let mock_provider = MockIndexDataProvider::new();

        // When Creating an Indexer
        let result = Indexer::new(
            nonexistent,
            mock_repo,
            &config,
            Box::new(mock_provider),
            ScanConfig::default(),
        );

        // Then It should fail with ScanFailed error
        assert!(matches!(result, Err(IndexingError::ScanFailed { .. })));
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
        let mut mock_provider = MockIndexDataProvider::new();
        mock_provider
            .expect_generate_batch()
            .returning(|_| Ok(vec![Embedding::try_new(vec![0.1; 384]).unwrap()]));

        let mut indexer = Indexer::new(
            temp_dir.path(),
            mock_repo,
            &config,
            Box::new(mock_provider),
            ScanConfig::default(),
        )
        .unwrap();

        // When Building the index
        let result = indexer.index(IndexStrategy::Build);

        // Then It should succeed and report indexed files
        assert!(result.is_ok());
        let index_result = result.unwrap();
        assert_eq!(index_result.files_added, 1);
        assert_eq!(index_result.files_processed, 1);
        assert!(index_result.chunks_affected > 0);
        assert_eq!(index_result.mode, IndexMode::Best);
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
        let mut mock_provider = MockIndexDataProvider::new();
        mock_provider
            .expect_generate_batch()
            .returning(|_| Ok(vec![Embedding::try_new(vec![0.1; 384]).unwrap()]));

        let mut indexer = Indexer::new(
            temp_dir.path(),
            mock_repo,
            &config,
            Box::new(mock_provider),
            ScanConfig::default(),
        )
        .unwrap();

        // When Building the index
        let result = indexer.index(IndexStrategy::Build);

        // Then It should succeed and index the rust file
        assert!(result.is_ok());
        let index_result = result.unwrap();
        assert_eq!(index_result.files_added, 1);
        assert!(index_result.chunks_affected > 0);
    }

    #[test]
    fn test_build_calls_provider_generate() {
        // Given A forest with a markdown file
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("test.md"), "# Test\n\nContent").unwrap();

        let mut mock_repo = MockChunkRepository::new();
        mock_repo.expect_save_batch().times(1).returning(|_| Ok(()));

        let config = EmbeddingModelConfig::default();
        let mut mock_provider = MockIndexDataProvider::new();
        mock_provider
            .expect_generate_batch()
            .times(1)
            .returning(|_| Ok(vec![Embedding::try_new(vec![0.1; 384]).unwrap()]));

        let mut indexer = Indexer::new(
            temp_dir.path(),
            mock_repo,
            &config,
            Box::new(mock_provider),
            ScanConfig::default(),
        )
        .unwrap();

        // When Building the index
        let result = indexer.index(IndexStrategy::Build);

        // Then Provider generate should be called
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_propagates_provider_errors() {
        // Given A provider that fails to generate
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("test.md"), "# Test\n\nContent").unwrap();

        let mock_repo = MockChunkRepository::new();
        let config = EmbeddingModelConfig::default();
        let mut mock_provider = MockIndexDataProvider::new();
        mock_provider.expect_generate_batch().returning(|_| {
            Err(
                crate::knowledge::indexing::IndexDataError::EmbeddingFailed {
                    source: Box::new(std::io::Error::other("test error")),
                },
            )
        });

        let mut indexer = Indexer::new(
            temp_dir.path(),
            mock_repo,
            &config,
            Box::new(mock_provider),
            ScanConfig::default(),
        )
        .unwrap();

        // When Building the index
        let result = indexer.index(IndexStrategy::Build);

        // Then It should fail with IndexDataGenerationFailed error
        assert!(matches!(
            result,
            Err(IndexingError::IndexDataGenerationFailed { .. })
        ));
    }

    #[test]
    fn test_build_propagates_storage_errors() {
        // Given A repository that fails to store
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("test.md"), "# Test\n\nContent").unwrap();

        let mut mock_repo = MockChunkRepository::new();
        mock_repo.expect_save_batch().returning(|_| {
            Err(StorageError::InvalidData {
                message: "test error".to_string(),
            })
        });

        let config = EmbeddingModelConfig::default();
        let mut mock_provider = MockIndexDataProvider::new();
        mock_provider
            .expect_generate_batch()
            .returning(|_| Ok(vec![Embedding::try_new(vec![0.1; 384]).unwrap()]));

        let mut indexer = Indexer::new(
            temp_dir.path(),
            mock_repo,
            &config,
            Box::new(mock_provider),
            ScanConfig::default(),
        )
        .unwrap();

        // When Building the index
        let result = indexer.index(IndexStrategy::Build);

        // Then It should fail with StorageFailed error
        assert!(matches!(result, Err(IndexingError::StorageFailed { .. })));
    }

    #[test]
    fn test_build_handles_empty_forest() {
        // Given An empty forest directory
        let temp_dir = TempDir::new().unwrap();

        let mut mock_repo = MockChunkRepository::new();
        mock_repo.expect_save_batch().times(0);

        let config = EmbeddingModelConfig::default();
        let mut mock_provider = MockIndexDataProvider::new();

        let mut indexer = Indexer::new(
            temp_dir.path(),
            mock_repo,
            &config,
            Box::new(mock_provider),
            ScanConfig::default(),
        )
        .unwrap();

        // When Building the index
        let result = indexer.index(IndexStrategy::Build).unwrap();

        // Then It should succeed with zero files indexed
        assert_eq!(result.files_added, 0);
        assert_eq!(result.files_processed, 0);
        assert_eq!(result.chunks_affected, 0);
        assert_eq!(result.mode, IndexMode::Best);
    }

    #[test]
    fn test_build_uses_provider_mode_in_result() {
        // Given A provider with Best mode
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("test.md"), "# Test\n\nContent").unwrap();

        let mut mock_repo = MockChunkRepository::new();
        mock_repo.expect_save_batch().returning(|_| Ok(()));

        let config = EmbeddingModelConfig::default();
        let mut mock_provider = MockIndexDataProvider::new();
        mock_provider.expect_generate_batch().returning(|_| {
            Ok(vec![
                crate::knowledge::domain::Embedding::try_new(vec![0.1; 384]).unwrap(),
            ])
        });

        let mut indexer = Indexer::new(
            temp_dir.path(),
            mock_repo,
            &config,
            Box::new(mock_provider),
            ScanConfig::default(),
        )
        .unwrap();

        // When Building the index
        let result = indexer.index(IndexStrategy::Build).unwrap();

        // Then Result should reflect provider mode
        assert_eq!(result.mode, IndexMode::Best);
    }

    #[test]
    fn test_rebuild_clears_then_builds() {
        // Given A forest with files
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("test.md"), "# Test\n\nContent").unwrap();

        let mut mock_repo = MockChunkRepository::new();
        mock_repo.expect_clear().times(1).returning(|| Ok(5));
        mock_repo.expect_save_batch().times(1).returning(|_| Ok(()));

        let config = EmbeddingModelConfig::default();
        let mut mock_provider = MockIndexDataProvider::new();
        mock_provider
            .expect_generate_batch()
            .returning(|_| Ok(vec![Embedding::try_new(vec![0.1; 384]).unwrap()]));

        let mut indexer = Indexer::new(
            temp_dir.path(),
            mock_repo,
            &config,
            Box::new(mock_provider),
            ScanConfig::default(),
        )
        .unwrap();

        // When Rebuilding the index
        let result = indexer.index(IndexStrategy::Rebuild);

        // Then It should clear then build
        assert!(result.is_ok());
    }

    #[test]
    fn test_incremental_adds_new_files() {
        // Given A forest with a new file and empty index
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("new.md"), "# New\n\nContent").unwrap();

        let mut mock_repo = MockChunkRepository::new();
        mock_repo
            .expect_get_indexed_files()
            .returning(|| Ok(HashMap::new()));
        mock_repo.expect_save_batch().times(1).returning(|_| Ok(()));

        let config = EmbeddingModelConfig::default();
        let mut mock_provider = MockIndexDataProvider::new();
        mock_provider
            .expect_generate_batch()
            .returning(|_| Ok(vec![Embedding::try_new(vec![0.1; 384]).unwrap()]));

        let mut indexer = Indexer::new(
            temp_dir.path(),
            mock_repo,
            &config,
            Box::new(mock_provider),
            ScanConfig::default(),
        )
        .unwrap();

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
        let mut mock_provider = MockIndexDataProvider::new();
        mock_provider
            .expect_generate_batch()
            .returning(|_| Ok(vec![Embedding::try_new(vec![0.1; 384]).unwrap()]));

        let mut indexer = Indexer::new(
            temp_dir.path(),
            mock_repo,
            &config,
            Box::new(mock_provider),
            ScanConfig::default(),
        )
        .unwrap();

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
        let mut mock_provider = MockIndexDataProvider::new();

        let mut indexer = Indexer::new(
            temp_dir.path(),
            mock_repo,
            &config,
            Box::new(mock_provider),
            ScanConfig::default(),
        )
        .unwrap();

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
        let mut mock_provider = MockIndexDataProvider::new();
        mock_provider
            .expect_generate_batch()
            .returning(|_| Ok(vec![Embedding::try_new(vec![0.1; 384]).unwrap()]));

        let mut indexer = Indexer::new(
            temp_dir.path(),
            mock_repo,
            &config,
            Box::new(mock_provider),
            ScanConfig::default(),
        )
        .unwrap();

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
        let mock_provider = MockIndexDataProvider::new();

        let mut indexer = Indexer::new(
            temp_dir.path(),
            mock_repo,
            &config,
            Box::new(mock_provider),
            ScanConfig::default(),
        )
        .unwrap();

        // When Updating the index
        let result = indexer.index(IndexStrategy::Incremental);

        // Then It should fail with StorageFailed error
        assert!(matches!(result, Err(IndexingError::StorageFailed { .. })));
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
        let mut mock_provider = MockIndexDataProvider::new();

        let mut indexer = Indexer::new(
            temp_dir.path(),
            mock_repo,
            &config,
            Box::new(mock_provider),
            ScanConfig::default(),
        )
        .unwrap();

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
