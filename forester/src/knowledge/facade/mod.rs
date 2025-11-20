//! High-level facade for the knowledge index
//!
//! The [`KnowledgeIndex`] provides a unified interface for all knowledge index operations,
//! coordinating between the indexer, repository, and search engines.
//!
//! # Quick Start
//!
//! ```no_run
//! use forester::knowledge::KnowledgeIndex;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Open or create an index
//! let mut index = KnowledgeIndex::open("/path/to/forest")?;
//!
//! // Build the index
//! let result = index.build()?;
//! println!("Indexed {} files", result.files_processed);
//!
//! // Search
//! let results = index.search("how does boot work", 10)?;
//! for result in results.results {
//!     println!("Score: {}, File: {}", result.score, result.chunk.source.file_path);
//! }
//!
//! // Check status
//! let status = index.status()?;
//! println!("Index has {} chunks from {} files", status.chunk_count, status.file_count);
//! # Ok(())
//! # }
//! ```

mod types;

pub use types::{IndexError, IndexStatus};

use bon::Builder;
use snafu::ResultExt;
use std::path::{Path, PathBuf};

use crate::knowledge::domain::{EmbeddingModelConfig, SearchQuery, SearchResults};
use crate::knowledge::indexing::IndexResult;
use crate::knowledge::search::SearchEngine;
use crate::knowledge::storage::ChunkRepository;
use crate::knowledge::storage::sqlite::SqliteChunkRepository;

/// High-level interface for the knowledge index
///
/// Coordinates indexing, storage, and search operations. Owns the database
/// connection and provides a unified API for all knowledge index functionality.
#[derive(Builder)]
#[builder(on(_, into))]
pub struct KnowledgeIndex {
    forest_root: PathBuf,
    db_path: PathBuf,
    config: EmbeddingModelConfig,
    repository: SqliteChunkRepository,
}

impl KnowledgeIndex {
    /// Open or create a knowledge index with default configuration
    ///
    /// Creates the `.forester/` directory and `knowledge.db` database if they don't exist.
    pub fn open(forest_root: impl AsRef<Path>) -> Result<Self, IndexError> {
        Self::open_with_config(forest_root, EmbeddingModelConfig::default())
    }

    /// Open or create a knowledge index with custom configuration
    ///
    /// Like [`open`](Self::open), but allows specifying a custom embedding model configuration.
    /// If the index already exists, validates that its configuration matches.
    pub fn open_with_config(
        forest_root: impl AsRef<Path>,
        config: EmbeddingModelConfig,
    ) -> Result<Self, IndexError> {
        use types::index_error::*;

        let forest_root = forest_root.as_ref();

        if !forest_root.exists() {
            return Err(IndexError::ForestRootNotFound {
                path: forest_root.display().to_string(),
            });
        }

        let forester_dir = forest_root.join(".forester");
        if !forester_dir.exists() {
            std::fs::create_dir_all(&forester_dir).context(ForesterDirCreationFailedSnafu)?;
        }

        let db_path = Self::default_db_path(forest_root);
        let is_new_db = !db_path.exists();

        let mut repository =
            SqliteChunkRepository::open(&db_path, &config).context(DatabaseAccessFailedSnafu)?;

        if is_new_db {
            let metadata = crate::knowledge::domain::IndexMetadata::builder()
                .last_build(std::time::SystemTime::now())
                .chunk_count(0)
                .file_count(0)
                .model_config(config.clone())
                .build();
            repository
                .set_metadata(&metadata)
                .context(DatabaseAccessFailedSnafu)?;
        }

        let metadata = repository
            .get_metadata()
            .context(DatabaseAccessFailedSnafu)?;

        if metadata.model_config != config {
            return Err(IndexError::DatabaseAccessFailed {
                source: crate::knowledge::storage::StorageError::ConfigMismatch {
                    expected: config.clone(),
                    actual: metadata.model_config.clone(),
                },
            });
        }

        Ok(Self {
            forest_root: forest_root.to_path_buf(),
            db_path,
            config,
            repository,
        })
    }

    /// Build the index from scratch without clearing existing data
    ///
    /// Scans all files in the forest and indexes them. If chunks already exist,
    /// they are not removed. Use [`rebuild`](Self::rebuild) to start fresh.
    pub fn build(&mut self) -> Result<IndexResult, IndexError> {
        use types::index_error::*;

        let provider = self.create_provider()?;
        let scan_config = self.load_scan_config()?;
        let repository = SqliteChunkRepository::open(&self.db_path, &self.config)
            .context(DatabaseAccessFailedSnafu)?;

        let mut indexer = crate::knowledge::indexing::Indexer::new(
            &self.forest_root,
            repository,
            &self.config,
            provider,
            scan_config,
        )
        .context(IndexingFailedSnafu)?;

        let result = indexer
            .index(crate::knowledge::indexing::IndexStrategy::Build)
            .context(IndexingFailedSnafu)?;

        self.repository = SqliteChunkRepository::open(&self.db_path, &self.config)
            .context(DatabaseAccessFailedSnafu)?;

        Ok(result)
    }

    /// Clear the index and rebuild from scratch
    ///
    /// Removes all existing chunks, then scans and indexes all files in the forest.
    pub fn rebuild(&mut self) -> Result<IndexResult, IndexError> {
        use types::index_error::*;

        let provider = self.create_provider()?;
        let scan_config = self.load_scan_config()?;
        let repository = SqliteChunkRepository::open(&self.db_path, &self.config)
            .context(DatabaseAccessFailedSnafu)?;

        let mut indexer = crate::knowledge::indexing::Indexer::new(
            &self.forest_root,
            repository,
            &self.config,
            provider,
            scan_config,
        )
        .context(IndexingFailedSnafu)?;

        let result = indexer
            .index(crate::knowledge::indexing::IndexStrategy::Rebuild)
            .context(IndexingFailedSnafu)?;

        self.repository = SqliteChunkRepository::open(&self.db_path, &self.config)
            .context(DatabaseAccessFailedSnafu)?;

        Ok(result)
    }

    /// Update the index incrementally
    ///
    /// Only processes files that have been added, modified, or deleted since
    /// the last index operation. More efficient than a full rebuild.
    pub fn update(&mut self) -> Result<IndexResult, IndexError> {
        use types::index_error::*;

        let provider = self.create_provider()?;
        let scan_config = self.load_scan_config()?;
        let repository = SqliteChunkRepository::open(&self.db_path, &self.config)
            .context(DatabaseAccessFailedSnafu)?;

        let mut indexer = crate::knowledge::indexing::Indexer::new(
            &self.forest_root,
            repository,
            &self.config,
            provider,
            scan_config,
        )
        .context(IndexingFailedSnafu)?;

        let result = indexer
            .index(crate::knowledge::indexing::IndexStrategy::Incremental)
            .context(IndexingFailedSnafu)?;

        self.repository = SqliteChunkRepository::open(&self.db_path, &self.config)
            .context(DatabaseAccessFailedSnafu)?;

        Ok(result)
    }

    /// Remove all chunks from the index
    ///
    /// Returns the number of chunks removed.
    pub fn clear(&mut self) -> Result<usize, IndexError> {
        use types::index_error::*;

        self.repository.clear().context(DatabaseAccessFailedSnafu)
    }

    /// Search the index
    ///
    /// Executes a semantic search query using embeddings.
    /// The limit must be between 1 and 100.
    pub fn search(
        &self,
        query: impl AsRef<str>,
        limit: usize,
    ) -> Result<SearchResults, IndexError> {
        use crate::knowledge::domain::{QueryText, ResultLimit};
        use types::index_error::*;

        let query_text = query.as_ref();

        if query_text.is_empty() {
            return Err(IndexError::InvalidQuery {
                message: "Query cannot be empty".to_string(),
            });
        }

        if !(1..=100).contains(&limit) {
            return Err(IndexError::InvalidResultLimit { limit });
        }

        let search_query = SearchQuery::builder()
            .text(
                QueryText::try_new(query_text).map_err(|e| IndexError::InvalidQuery {
                    message: e.to_string(),
                })?,
            )
            .limit(
                ResultLimit::try_new(limit)
                    .map_err(|_| IndexError::InvalidResultLimit { limit })?,
            )
            .build();

        let engine = self.create_search_engine()?;
        engine.search(&search_query).context(SearchFailedSnafu)
    }

    /// Get index status and statistics
    ///
    /// Returns metadata about the index including chunk count, file count,
    /// last build time, and disk size.
    pub fn status(&self) -> Result<IndexStatus, IndexError> {
        use types::index_error::*;

        let metadata = self
            .repository
            .get_metadata()
            .context(DatabaseAccessFailedSnafu)?;

        let size_bytes = std::fs::metadata(&self.db_path).map(|m| m.len()).ok();

        Ok(IndexStatus::builder()
            .exists(true)
            .chunk_count(metadata.chunk_count)
            .file_count(metadata.file_count)
            .last_build(metadata.last_build)
            .model_config(self.config.clone())
            .maybe_size_bytes(size_bytes)
            .build())
    }

    /// Get the forest root path
    pub fn forest_root(&self) -> &Path {
        &self.forest_root
    }

    /// Get the database path
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// Get the embedding model configuration
    pub fn config(&self) -> &EmbeddingModelConfig {
        &self.config
    }

    /// Compute the default database path for a forest root
    ///
    /// Returns `<forest_root>/.forester/knowledge.db`
    fn default_db_path(forest_root: impl AsRef<Path>) -> PathBuf {
        forest_root.as_ref().join(".forester/knowledge.db")
    }

    /// Create a search engine for the current index mode
    fn create_search_engine(&self) -> Result<Box<dyn SearchEngine>, IndexError> {
        use types::index_error::*;

        let repo = SqliteChunkRepository::open(&self.db_path, &self.config)
            .context(DatabaseAccessFailedSnafu)?;
        let embedding_model = crate::knowledge::search::EmbeddingModel::builder()
            .model_name(self.config.model_name.clone())
            .dimension(self.config.embedding_dim)
            .cache_dir(self.forest_root.join(".forester/cache/model"))
            .build()
            .load()
            .context(EmbeddingProviderCreationFailedSnafu)?;
        Ok(Box::new(
            crate::knowledge::search::SemanticSearchEngine::new(repo, Box::new(embedding_model)),
        ))
    }

    fn create_provider(
        &self,
    ) -> Result<Box<dyn crate::knowledge::indexing::IndexDataProvider>, IndexError> {
        use types::index_error::*;

        let embedding_model = crate::knowledge::search::EmbeddingModel::builder()
            .model_name(self.config.model_name.clone())
            .dimension(self.config.embedding_dim)
            .cache_dir(self.forest_root.join(".forester/cache/model"))
            .build()
            .load()
            .context(EmbeddingProviderCreationFailedSnafu)?;
        Ok(Box::new(
            crate::knowledge::indexing::provider::EmbeddingDataProvider::new(Box::new(
                embedding_model,
            )),
        ))
    }

    /// Load scan configuration from `.forester.toml` if it exists
    fn load_scan_config(&self) -> Result<crate::knowledge::domain::ScanConfig, IndexError> {
        use types::index_error::*;

        let forester_config = crate::knowledge::indexing::load_forester_config(&self.forest_root)
            .context(ConfigLoadFailedSnafu)?;

        let targets = forester_config.map(|c| c.targets).unwrap_or_default();

        Ok(crate::knowledge::domain::ScanConfig::builder()
            .targets(targets)
            .build())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_open_creates_forester_directory() {
        // Given A forest root without .forester directory
        let temp_dir = TempDir::new().unwrap();
        let forest_root = temp_dir.path();

        // When Opening an index
        let result = KnowledgeIndex::open(forest_root);

        // Then It should create .forester directory
        assert!(result.is_ok());
        assert!(forest_root.join(".forester").exists());
    }

    #[test]
    fn test_open_creates_database_file() {
        // Given A forest root without existing database
        let temp_dir = TempDir::new().unwrap();
        let forest_root = temp_dir.path();

        // When Opening an index
        let result = KnowledgeIndex::open(forest_root);

        // Then It should create knowledge.db
        assert!(result.is_ok());
        assert!(forest_root.join(".forester/knowledge.db").exists());
    }

    #[test]
    fn test_open_with_nonexistent_forest_root_fails() {
        // Given A nonexistent forest root
        let nonexistent = std::path::Path::new("/nonexistent/forest");

        // When Opening an index
        let result = KnowledgeIndex::open(nonexistent);

        // Then It should fail with ForestRootNotFound
        assert!(matches!(result, Err(IndexError::ForestRootNotFound { .. })));
    }

    #[test]
    fn test_open_returns_index_with_correct_paths() {
        // Given A forest root
        let temp_dir = TempDir::new().unwrap();
        let forest_root = temp_dir.path();

        // When Opening an index
        let index = KnowledgeIndex::open(forest_root).unwrap();

        // Then Paths should be correct
        assert_eq!(index.forest_root(), forest_root);
        assert_eq!(index.db_path(), forest_root.join(".forester/knowledge.db"));
    }

    #[test]
    fn test_open_with_existing_index_same_mode_succeeds() {
        // Given An existing index
        let temp_dir = TempDir::new().unwrap();
        let _index = KnowledgeIndex::open(temp_dir.path()).unwrap();

        // When Opening with same mode
        let result = KnowledgeIndex::open(temp_dir.path());

        // Then It should succeed
        assert!(result.is_ok());
    }

    #[test]
    fn test_open_with_config_uses_custom_config() {
        // Given A custom embedding config
        let temp_dir = TempDir::new().unwrap();
        let custom_config = EmbeddingModelConfig::builder()
            .model_name("custom-model")
            .embedding_dim(512)
            .max_tokens(512)
            .overlap_tokens(50)
            .build();

        // When Opening with custom config
        let index =
            KnowledgeIndex::open_with_config(temp_dir.path(), custom_config.clone()).unwrap();

        // Then Index should use custom config
        assert_eq!(index.config(), &custom_config);
    }

    #[test]
    fn test_open_with_config_validates_existing_config() {
        // Given An existing index with default config
        let temp_dir = TempDir::new().unwrap();
        let _index = KnowledgeIndex::open(temp_dir.path()).unwrap();

        // When Opening with different config
        let different_config = EmbeddingModelConfig::builder()
            .model_name("different-model")
            .embedding_dim(512)
            .max_tokens(512)
            .overlap_tokens(50)
            .build();
        let result = KnowledgeIndex::open_with_config(temp_dir.path(), different_config);

        // Then It should fail with DatabaseAccessFailed (config mismatch)
        assert!(matches!(
            result,
            Err(IndexError::DatabaseAccessFailed { .. })
        ));
    }

    #[test]
    fn test_build_indexes_files_in_forest() {
        // Given A forest with markdown files
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("README.md"), "# Test\n\nContent here").unwrap();

        let mut index = KnowledgeIndex::open(temp_dir.path()).unwrap();

        // When Building the index
        let result = index.build();

        // Then It should succeed and report indexed files
        assert!(result.is_ok());
        let index_result = result.unwrap();
        assert!(index_result.files_processed > 0);
        assert!(index_result.chunks_affected > 0);
    }

    #[test]
    fn test_build_handles_empty_forest() {
        // Given An empty forest
        let temp_dir = TempDir::new().unwrap();
        let mut index = KnowledgeIndex::open(temp_dir.path()).unwrap();

        // When Building
        let result = index.build().unwrap();

        // Then It should succeed with zero files
        assert_eq!(result.files_processed, 0);
        assert_eq!(result.chunks_affected, 0);
    }

    #[test]
    fn test_rebuild_clears_existing_chunks() {
        // Given An index with existing chunks
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("test.md"), "# Test\n\nContent").unwrap();

        let mut index = KnowledgeIndex::open(temp_dir.path()).unwrap();
        index.build().unwrap();

        // When Rebuilding
        let result = index.rebuild();

        // Then It should succeed
        assert!(result.is_ok());
    }

    #[test]
    fn test_rebuild_reindexes_all_files() {
        // Given An index
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("test.md"), "# Test\n\nContent").unwrap();

        let mut index = KnowledgeIndex::open(temp_dir.path()).unwrap();

        // When Rebuilding
        let result = index.rebuild().unwrap();

        // Then It should index files
        assert!(result.files_processed > 0);
        assert!(result.chunks_affected > 0);
    }

    #[test]
    fn test_update_detects_new_files() {
        // Given An index with no files
        let temp_dir = TempDir::new().unwrap();
        let mut index = KnowledgeIndex::open(temp_dir.path()).unwrap();
        index.build().unwrap();

        // When Adding a new file and updating
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("new.md"), "# New\n\nContent").unwrap();

        let result = index.update().unwrap();

        // Then It should report one file added
        assert_eq!(result.files_added, 1);
        assert!(result.chunks_affected > 0);
    }

    #[test]
    fn test_update_detects_deleted_files() {
        // Given An index with a file
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        let file_path = repo_dir.join("test.md");
        fs::write(&file_path, "# Test\n\nContent").unwrap();

        let mut index = KnowledgeIndex::open(temp_dir.path()).unwrap();
        index.build().unwrap();

        // When Deleting the file and updating
        fs::remove_file(&file_path).unwrap();

        let result = index.update().unwrap();

        // Then It should report one file removed
        assert_eq!(result.files_removed, 1);
    }

    #[test]
    fn test_clear_removes_all_chunks() {
        // Given An index with chunks
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("test.md"), "# Test\n\nContent").unwrap();

        let mut index = KnowledgeIndex::open(temp_dir.path()).unwrap();
        index.build().unwrap();

        // When Clearing the index
        let result = index.clear();

        // Then It should succeed and return count
        assert!(result.is_ok());
        assert!(result.unwrap() > 0);
    }

    #[test]
    fn test_clear_on_empty_index_returns_zero() {
        // Given An empty index
        let temp_dir = TempDir::new().unwrap();
        let mut index = KnowledgeIndex::open(temp_dir.path()).unwrap();

        // When Clearing
        let result = index.clear().unwrap();

        // Then It should return zero
        assert_eq!(result, 0);
    }

    #[test]
    fn test_search_executes_query() {
        // Given An index with content
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("test.md"), "# Boot Process\n\nHow boot works").unwrap();

        let mut index = KnowledgeIndex::open(temp_dir.path()).unwrap();
        index.build().unwrap();

        // When Searching
        let result = index.search("boot", 10);

        // Then It should return results
        assert!(result.is_ok());
        let search_results = result.unwrap();
        assert!(!search_results.results.is_empty());
    }

    #[test]
    fn test_search_respects_limit() {
        // Given An index with multiple chunks
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(
            repo_dir.join("test.md"),
            "# Test\n\ntest test test\n\n## Section\n\ntest test",
        )
        .unwrap();

        let mut index = KnowledgeIndex::open(temp_dir.path()).unwrap();
        index.build().unwrap();

        // When Searching with limit 2
        let result = index.search("test", 2).unwrap();

        // Then Results should respect limit
        assert!(result.results.len() <= 2);
    }

    #[test]
    fn test_search_validates_limit_minimum() {
        // Given An index
        let temp_dir = TempDir::new().unwrap();
        let index = KnowledgeIndex::open(temp_dir.path()).unwrap();

        // When Searching with limit 0
        let result = index.search("test", 0);

        // Then It should fail with InvalidResultLimit
        assert!(matches!(result, Err(IndexError::InvalidResultLimit { .. })));
    }

    #[test]
    fn test_search_validates_limit_maximum() {
        // Given An index
        let temp_dir = TempDir::new().unwrap();
        let index = KnowledgeIndex::open(temp_dir.path()).unwrap();

        // When Searching with limit 101
        let result = index.search("test", 101);

        // Then It should fail with InvalidResultLimit
        assert!(matches!(result, Err(IndexError::InvalidResultLimit { .. })));
    }

    #[test]
    fn test_search_validates_empty_query() {
        // Given An index
        let temp_dir = TempDir::new().unwrap();
        let index = KnowledgeIndex::open(temp_dir.path()).unwrap();

        // When Searching with empty query
        let result = index.search("", 10);

        // Then It should fail with InvalidQuery
        assert!(matches!(result, Err(IndexError::InvalidQuery { .. })));
    }

    #[test]
    fn test_status_returns_index_metadata() {
        // Given An index with content
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("test.md"), "# Test\n\nContent").unwrap();

        let mut index = KnowledgeIndex::open(temp_dir.path()).unwrap();
        index.build().unwrap();

        // When Getting status
        let result = index.status();

        // Then It should return status with metadata
        assert!(result.is_ok());
        let status = result.unwrap();
        assert!(status.exists);
        assert!(status.chunk_count > 0);
        assert!(status.file_count > 0);
        assert!(status.last_build.is_some());
    }

    #[test]
    fn test_status_on_empty_index() {
        // Given An empty index
        let temp_dir = TempDir::new().unwrap();
        let index = KnowledgeIndex::open(temp_dir.path()).unwrap();

        // When Getting status
        let status = index.status().unwrap();

        // Then It should show zero counts
        assert!(status.exists);
        assert_eq!(status.chunk_count, 0);
        assert_eq!(status.file_count, 0);
    }

    #[test]
    fn test_status_includes_disk_size() {
        // Given An index with content
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("test.md"), "# Test\n\nContent").unwrap();

        let mut index = KnowledgeIndex::open(temp_dir.path()).unwrap();
        index.build().unwrap();

        // When Getting status
        let status = index.status().unwrap();

        // Then It should include size
        assert!(status.size_bytes.is_some());
        assert!(status.size_bytes.unwrap() > 0);
    }

    #[test]
    fn test_default_db_path_returns_correct_path() {
        // Given A forest root
        let forest_root = std::path::Path::new("/test/forest");

        // When Computing default db path
        let db_path = KnowledgeIndex::default_db_path(forest_root);

        // Then It should be .forester/knowledge.db
        assert_eq!(db_path, forest_root.join(".forester/knowledge.db"));
    }

    #[test]
    fn test_forest_root_returns_correct_path() {
        // Given An index
        let temp_dir = TempDir::new().unwrap();
        let forest_root = temp_dir.path();
        let index = KnowledgeIndex::open(forest_root).unwrap();

        // When Getting forest root
        let root = index.forest_root();

        // Then It should match original path
        assert_eq!(root, forest_root);
    }

    #[test]
    fn test_db_path_returns_correct_path() {
        // Given An index
        let temp_dir = TempDir::new().unwrap();
        let forest_root = temp_dir.path();
        let index = KnowledgeIndex::open(forest_root).unwrap();

        // When Getting db path
        let db_path = index.db_path();

        // Then It should be .forester/knowledge.db
        assert_eq!(db_path, forest_root.join(".forester/knowledge.db"));
    }

    #[test]
    fn test_config_returns_embedding_config() {
        // Given An index with custom config
        let temp_dir = TempDir::new().unwrap();
        let custom_config = EmbeddingModelConfig::builder()
            .model_name("test-model")
            .embedding_dim(384)
            .max_tokens(256)
            .overlap_tokens(38)
            .build();

        let index =
            KnowledgeIndex::open_with_config(temp_dir.path(), custom_config.clone()).unwrap();

        // When Getting config
        let config = index.config();

        // Then It should return the custom config
        assert_eq!(config, &custom_config);
    }

    #[test]
    fn test_load_scan_config_with_existing_forester_toml() {
        // Given A forest root with .forester.toml containing targets
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"
targets = ["docs", "bottlerocket"]
"#;
        fs::write(temp_dir.path().join(".forester.toml"), config_content).unwrap();

        let index = KnowledgeIndex::open(temp_dir.path()).unwrap();

        // When Loading scan config
        let scan_config = index.load_scan_config().unwrap();

        // Then It should return ScanConfig with those targets
        assert_eq!(scan_config.targets.len(), 2);
        assert_eq!(scan_config.targets[0], PathBuf::from("docs"));
        assert_eq!(scan_config.targets[1], PathBuf::from("bottlerocket"));
    }

    #[test]
    fn test_load_scan_config_without_forester_toml() {
        // Given A forest root without .forester.toml
        let temp_dir = TempDir::new().unwrap();
        let index = KnowledgeIndex::open(temp_dir.path()).unwrap();

        // When Loading scan config
        let scan_config = index.load_scan_config().unwrap();

        // Then It should return ScanConfig with empty targets
        assert!(scan_config.targets.is_empty());
    }

    #[test]
    fn test_build_uses_configured_targets() {
        // Given A forest with .forester.toml specifying specific targets
        let temp_dir = TempDir::new().unwrap();
        // Create .git at forest root so ignore crate works properly
        fs::create_dir(temp_dir.path().join(".git")).unwrap();
        let docs_dir = temp_dir.path().join("docs");
        let bottlerocket_dir = temp_dir.path().join("bottlerocket");
        let ignored_dir = temp_dir.path().join("ignored");
        fs::create_dir(&docs_dir).unwrap();
        fs::create_dir(&bottlerocket_dir).unwrap();
        fs::create_dir(&ignored_dir).unwrap();
        fs::write(
            docs_dir.join("guide.md"),
            "# Guide\n\nDocumentation content",
        )
        .unwrap();
        fs::write(
            bottlerocket_dir.join("README.md"),
            "# Bottlerocket\n\nProject info",
        )
        .unwrap();
        fs::write(ignored_dir.join("secret.md"), "# Secret\n\nSecret content").unwrap();

        let config_content = r#"
targets = ["docs", "bottlerocket"]
"#;
        fs::write(temp_dir.path().join(".forester.toml"), config_content).unwrap();

        let mut index = KnowledgeIndex::open(temp_dir.path()).unwrap();

        // When Building the index
        let result = index.build().unwrap();

        // Then It should only index files from configured targets
        assert_eq!(result.files_processed, 2);
        let status = index.status().unwrap();
        assert_eq!(status.file_count, 2);
    }

    #[test]
    fn test_rebuild_uses_configured_targets() {
        // Given A forest with configured targets
        let temp_dir = TempDir::new().unwrap();
        fs::create_dir(temp_dir.path().join(".git")).unwrap();
        let docs_dir = temp_dir.path().join("docs");
        let other_dir = temp_dir.path().join("other");
        fs::create_dir(&docs_dir).unwrap();
        fs::create_dir(&other_dir).unwrap();
        fs::write(docs_dir.join("guide.md"), "# Guide\n\nDocumentation").unwrap();
        fs::write(other_dir.join("other.md"), "# Other\n\nOther content").unwrap();

        let config_content = r#"
targets = ["docs"]
"#;
        fs::write(temp_dir.path().join(".forester.toml"), config_content).unwrap();

        let mut index = KnowledgeIndex::open(temp_dir.path()).unwrap();

        // When Rebuilding
        let result = index.rebuild().unwrap();

        // Then It should only index configured targets
        assert_eq!(result.files_processed, 1);
    }

    #[test]
    fn test_update_uses_configured_targets() {
        // Given A forest with configured targets
        let temp_dir = TempDir::new().unwrap();
        let docs_dir = temp_dir.path().join("docs");
        let other_dir = temp_dir.path().join("other");
        fs::create_dir(&docs_dir).unwrap();
        fs::create_dir(&other_dir).unwrap();

        let config_content = r#"
targets = ["docs"]
"#;
        fs::write(temp_dir.path().join(".forester.toml"), config_content).unwrap();

        let mut index = KnowledgeIndex::open(temp_dir.path()).unwrap();
        index.build().unwrap();

        // When Adding files to both directories and updating
        fs::write(docs_dir.join("new.md"), "# New").unwrap();
        fs::write(other_dir.join("ignored.md"), "# Ignored").unwrap();

        let result = index.update().unwrap();

        // Then It should only detect changes in configured targets
        assert_eq!(result.files_added, 1);
    }
}
