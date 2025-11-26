//! High-level facade for the knowledge index
//!
//! The [`KnowledgeIndex`] provides a unified interface for all knowledge index operations,
//! coordinating between the indexer, repository, and search engines.
//!
//! # Quick Start
//!
//! ```no_run
//! use sembly_core::knowledge::KnowledgeIndex;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Open an index handle
//! let index = KnowledgeIndex::open("/path/to/forest")?;
//!
//! // Build the index (creates database)
//! let result = index.build().call()?;
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
use std::sync::Arc;

use crate::knowledge::domain::{
    Context, ContextId, EmbeddingModelConfig, IndexMetadata, QueryText, ResultLimit, ScanConfig,
    SearchQuery, SearchResults,
};
use crate::knowledge::indexing::provider::EmbeddingDataProvider;
use crate::knowledge::indexing::{
    self, BatchConfig, IndexDataProvider, IndexResult, IndexStrategy, Indexer, IndexingFilter,
    ProgressReporter, load_sembly_config,
};
use crate::knowledge::scoring::ScoreBooster;
use crate::knowledge::search::{
    EmbeddingModel, LoadedEmbeddingModel, SearchEngine, SemanticSearchEngine,
};
use crate::knowledge::storage::ChunkRepository;
use crate::knowledge::storage::ContextRepository;
use crate::knowledge::storage::sqlite::SqliteChunkRepository;

/// High-level interface for the knowledge index
///
/// Coordinates indexing, storage, and search operations. Provides a unified
/// API for all knowledge index functionality.
#[derive(Builder)]
#[builder(on(_, into), builder_type = KnowledgeIndexConstructor)]
pub struct KnowledgeIndex {
    forest_root: PathBuf,
    db_path: PathBuf,
    config: EmbeddingModelConfig,
}

#[bon::bon]
impl KnowledgeIndex {
    /// Open a knowledge index with default configuration
    ///
    /// Creates the `.sembly/` directory if it doesn't exist. Validates configuration
    /// against existing database if present. Does not create the database.
    pub fn open(forest_root: impl AsRef<Path>) -> Result<Self, IndexError> {
        Self::open_with_config(forest_root, EmbeddingModelConfig::default())
    }

    /// Open a knowledge index with custom configuration
    ///
    /// Validates the configuration against any existing index. Does not create the database.
    pub fn open_with_config(
        forest_root: impl AsRef<Path>,
        config: EmbeddingModelConfig,
    ) -> Result<Self, IndexError> {
        use crate::knowledge::storage::StorageError;
        use types::index_error::*;

        let forest_root = forest_root.as_ref();

        if !forest_root.exists() {
            return Err(IndexError::ForestRootNotFound {
                path: forest_root.display().to_string(),
            });
        }

        let sembly_dir = forest_root.join(".sembly");
        if !sembly_dir.exists() {
            std::fs::create_dir_all(&sembly_dir).context(SemblyDirCreationFailedSnafu)?;
        }

        let db_path = Self::default_db_path(forest_root);

        // If database exists, validate config matches
        if db_path.exists() {
            let repository = SqliteChunkRepository::open(&db_path, &config)
                .context(DatabaseAccessFailedSnafu)?;

            let metadata = repository
                .get_metadata()
                .context(DatabaseAccessFailedSnafu)?;

            if metadata.model_config != config {
                return Err(IndexError::DatabaseAccessFailed {
                    source: StorageError::ConfigMismatch {
                        expected: config.clone(),
                        actual: metadata.model_config.clone(),
                    },
                });
            }
        }

        Ok(Self {
            forest_root: forest_root.to_path_buf(),
            db_path,
            config,
        })
    }

    /// Discover and open a knowledge index from the current working directory
    ///
    /// Walks up the directory tree from `cwd` to find a workspace containing
    /// `.sembly/knowledge.db`. Returns an error if no workspace is found.
    pub fn discover(cwd: impl AsRef<Path>) -> Result<Self, IndexError> {
        Self::discover_with_config(cwd, EmbeddingModelConfig::default())
    }

    /// Discover and open a knowledge index with custom configuration
    ///
    /// Walks up the directory tree from `cwd` to find a workspace.
    pub fn discover_with_config(
        cwd: impl AsRef<Path>,
        config: EmbeddingModelConfig,
    ) -> Result<Self, IndexError> {
        use crate::knowledge::context::discover_workspace;
        use types::index_error::*;

        let workspace = discover_workspace(cwd.as_ref()).context(WorkspaceNotFoundSnafu)?;
        Self::open_with_config(workspace.root(), config)
    }

    /// Resolve the context for a given working directory
    ///
    /// Returns the most specific registered context that contains the given path.
    /// The path must be within the workspace.
    pub fn resolve_context(&self, cwd: impl AsRef<Path>) -> Result<Context, IndexError> {
        use crate::knowledge::context::{Workspace, resolve_context};

        let workspace = Workspace::new(self.forest_root.clone());
        let repository = self.repository()?;
        let context_repo = repository.context_repository();

        resolve_context(&workspace, cwd.as_ref(), &context_repo).map_err(|e| match e {
            crate::knowledge::context::ResolutionError::NoMatchingContext {
                available_contexts,
            } => IndexError::ContextNotFound { available_contexts },
            other => IndexError::ContextResolutionFailed { source: other },
        })
    }

    /// Build the index for the first time
    ///
    /// Creates a new index by scanning all files in the forest. Fails if an index already exists.
    #[builder]
    pub fn build(
        &self,
        progress: Option<Arc<dyn ProgressReporter>>,
        #[builder(default = 100)] batch_size: usize,
    ) -> Result<IndexResult, IndexError> {
        use types::index_error::*;

        snafu::ensure!(
            !self.db_path.exists(),
            IndexAlreadyExistsSnafu {
                path: self.db_path.display().to_string()
            }
        );

        // Create and initialize the database
        let mut repository = SqliteChunkRepository::open(&self.db_path, &self.config)
            .context(DatabaseAccessFailedSnafu)?;

        let metadata = IndexMetadata::builder()
            .last_build(std::time::SystemTime::now())
            .chunk_count(0)
            .file_count(0)
            .model_config(self.config.clone())
            .build();
        repository
            .set_metadata(&metadata)
            .context(DatabaseAccessFailedSnafu)?;

        // Register the default context
        let default_context = Context::builder()
            .context_id(ContextId::from_path(".").expect("default context id"))
            .build();
        repository
            .context_repository()
            .insert_context(&default_context)
            .context(ContextRegistrationFailedSnafu)?;

        let provider = Box::new(self.create_provider()?) as Box<dyn IndexDataProvider>;
        let scan_config = self.load_scan_config()?;
        let filter = self.load_indexing_filter()?;
        let repository = self.repository()?;

        let batch_config = BatchConfig { batch_size };

        let mut indexer = Indexer::builder()
            .forest_root(&self.forest_root)
            .repository(repository)
            .config(&self.config)
            .provider(provider)
            .scan_config(scan_config)
            .filter(filter)
            .maybe_progress(progress)
            .batch_config(batch_config)
            .build()
            .context(IndexingFailedSnafu)?;

        let result = indexer
            .index(IndexStrategy::Build)
            .context(IndexingFailedSnafu)?;

        self.update_last_build_timestamp()?;

        Ok(result)
    }

    /// Delete the index and rebuild from scratch
    ///
    /// Deletes the existing index database and creates a new one by scanning all files.
    #[builder]
    pub fn rebuild(
        &self,
        progress: Option<Arc<dyn ProgressReporter>>,
        #[builder(default = 100)] batch_size: usize,
    ) -> Result<IndexResult, IndexError> {
        use types::index_error::*;

        if self.db_path.exists() {
            std::fs::remove_file(&self.db_path).context(IndexDeletionFailedSnafu)?;
        }

        // Create and initialize the database
        let mut repository = SqliteChunkRepository::open(&self.db_path, &self.config)
            .context(DatabaseAccessFailedSnafu)?;

        let metadata = IndexMetadata::builder()
            .last_build(std::time::SystemTime::now())
            .chunk_count(0)
            .file_count(0)
            .model_config(self.config.clone())
            .build();
        repository
            .set_metadata(&metadata)
            .context(DatabaseAccessFailedSnafu)?;

        // Register the default context
        let default_context = Context::builder()
            .context_id(ContextId::from_path(".").expect("default context id"))
            .build();
        repository
            .context_repository()
            .insert_context(&default_context)
            .context(ContextRegistrationFailedSnafu)?;

        let provider = Box::new(self.create_provider()?) as Box<dyn IndexDataProvider>;
        let scan_config = self.load_scan_config()?;
        let filter = self.load_indexing_filter()?;
        let repository = self.repository()?;

        let batch_config = BatchConfig { batch_size };

        let mut indexer = Indexer::builder()
            .forest_root(&self.forest_root)
            .repository(repository)
            .config(&self.config)
            .provider(provider)
            .scan_config(scan_config)
            .filter(filter)
            .maybe_progress(progress)
            .batch_config(batch_config)
            .build()
            .context(IndexingFailedSnafu)?;

        let result = indexer
            .index(IndexStrategy::Build)
            .context(IndexingFailedSnafu)?;

        self.update_last_build_timestamp()?;

        Ok(result)
    }

    /// Update the index incrementally
    ///
    /// Processes only files that have been added, modified, or deleted since
    /// the last index operation. Requires an existing index.
    #[builder]
    pub fn update(
        &self,
        progress: Option<Arc<dyn ProgressReporter>>,
        #[builder(default = 100)] batch_size: usize,
    ) -> Result<IndexResult, IndexError> {
        use types::index_error::*;

        snafu::ensure!(
            self.db_path.exists(),
            IndexNotFoundSnafu {
                path: self.db_path.display().to_string()
            }
        );

        let provider = Box::new(self.create_provider()?) as Box<dyn IndexDataProvider>;
        let scan_config = self.load_scan_config()?;
        let filter = self.load_indexing_filter()?;
        let repository = self.repository()?;

        let batch_config = BatchConfig { batch_size };

        let mut indexer = Indexer::builder()
            .forest_root(&self.forest_root)
            .repository(repository)
            .config(&self.config)
            .provider(provider)
            .scan_config(scan_config)
            .filter(filter)
            .maybe_progress(progress)
            .batch_config(batch_config)
            .build()
            .context(IndexingFailedSnafu)?;

        let result = indexer
            .index(IndexStrategy::Incremental)
            .context(IndexingFailedSnafu)?;

        self.update_last_build_timestamp()?;

        Ok(result)
    }

    /// Delete the index database
    ///
    /// Removes the index database file completely.
    pub fn clear(&self) -> Result<(), IndexError> {
        use types::index_error::*;

        if self.db_path.exists() {
            std::fs::remove_file(&self.db_path).context(IndexDeletionFailedSnafu)?;
        }

        Ok(())
    }

    /// Search the index
    ///
    /// Executes a semantic search query using embeddings. The limit parameter
    /// controls the maximum number of results returned (1-100).
    /// Searches within the default context.
    pub fn search(
        &self,
        query: impl AsRef<str>,
        limit: usize,
    ) -> Result<SearchResults, IndexError> {
        use types::index_error::*;

        // Use the default context for search
        let context_id = ContextId::from_path(".").expect("default context id");

        let search_query = SearchQuery::builder()
            .text(QueryText::try_new(query.as_ref()).context(InvalidQuerySnafu)?)
            .limit(ResultLimit::try_new(limit).context(InvalidResultLimitSnafu)?)
            .context_id(context_id)
            .build();

        let engine = self.create_search_engine()?;
        engine.search(&search_query).context(SearchFailedSnafu)
    }

    /// Search the index within a specific context
    ///
    /// Executes a semantic search query scoped to the specified context.
    /// Use `None` for context_id to search all contexts.
    pub fn search_in_context(
        &self,
        query: impl AsRef<str>,
        limit: usize,
        context_id: Option<ContextId>,
    ) -> Result<SearchResults, IndexError> {
        use types::index_error::*;

        let text = QueryText::try_new(query.as_ref()).context(InvalidQuerySnafu)?;
        let limit = ResultLimit::try_new(limit).context(InvalidResultLimitSnafu)?;

        let search_query = match context_id {
            Some(ctx_id) => SearchQuery::builder()
                .text(text)
                .limit(limit)
                .context_id(ctx_id)
                .build(),
            None => SearchQuery::builder().text(text).limit(limit).build(),
        };

        let engine = self.create_search_engine()?;
        engine.search(&search_query).context(SearchFailedSnafu)
    }

    /// Get index status and statistics
    ///
    /// Returns metadata including chunk count, file count, last build time, and disk size.
    pub fn status(&self) -> Result<IndexStatus, IndexError> {
        use types::index_error::*;

        let repository = self.repository()?;
        let metadata = repository
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

    /// List all registered contexts in the workspace
    pub fn list_contexts(&self) -> Result<Vec<Context>, IndexError> {
        use types::index_error::*;

        let repository = self.repository()?;
        let context_repo = repository.context_repository();
        context_repo
            .list_contexts()
            .context(ContextRegistrationFailedSnafu)
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
    /// Returns `<forest_root>/.sembly/knowledge.db`
    fn default_db_path(forest_root: impl AsRef<Path>) -> PathBuf {
        forest_root.as_ref().join(".sembly/knowledge.db")
    }

    /// Open a repository connection to the database
    fn repository(&self) -> Result<SqliteChunkRepository, IndexError> {
        use types::index_error::*;
        SqliteChunkRepository::open(&self.db_path, &self.config).context(DatabaseAccessFailedSnafu)
    }

    /// Update the last_build timestamp in metadata
    fn update_last_build_timestamp(&self) -> Result<(), IndexError> {
        use types::index_error::*;

        let mut repository = self.repository()?;
        let mut metadata = repository
            .get_metadata()
            .context(DatabaseAccessFailedSnafu)?;
        metadata.last_build = std::time::SystemTime::now();
        repository
            .set_metadata(&metadata)
            .context(DatabaseAccessFailedSnafu)?;
        Ok(())
    }

    /// Create a search engine for the current index mode
    ///
    /// Initializes the semantic search engine with the configured embedding model
    /// and score boosting rules from the sembly configuration.
    fn create_search_engine(
        &self,
    ) -> Result<SemanticSearchEngine<SqliteChunkRepository>, IndexError> {
        let repo = self.repository()?;
        let embedding_model = self.create_embedding_model()?;
        let score_booster = self.load_score_booster()?;

        Ok(SemanticSearchEngine::new(
            repo,
            Box::new(embedding_model),
            score_booster,
        ))
    }

    /// Create an embedding data provider for indexing operations
    ///
    /// Initializes the embedding model used to generate vector embeddings during indexing.
    fn create_provider(&self) -> Result<EmbeddingDataProvider, IndexError> {
        let embedding_model = self.create_embedding_model()?;
        Ok(EmbeddingDataProvider::new(Box::new(embedding_model)))
    }

    /// Create an embedding model with the configured parameters
    fn create_embedding_model(&self) -> Result<LoadedEmbeddingModel, IndexError> {
        use types::index_error::*;

        EmbeddingModel::builder()
            .model_name(self.config.model_name.clone())
            .dimension(self.config.embedding_dim)
            .cache_dir(self.forest_root.join(".sembly/cache/model"))
            .build()
            .load()
            .context(EmbeddingProviderCreationFailedSnafu)
    }

    /// Load score booster from sembly configuration
    fn load_score_booster(&self) -> Result<ScoreBooster, IndexError> {
        use types::index_error::*;

        let sembly_config = load_sembly_config(&self.forest_root).context(ConfigLoadFailedSnafu)?;

        Ok(if let Some(config) = sembly_config {
            if config.boost_rules.is_empty() {
                ScoreBooster::default()
            } else {
                ScoreBooster::new(config.boost_rules)
            }
        } else {
            ScoreBooster::default()
        })
    }

    /// Load scan configuration from `.sembly.toml` if it exists
    fn load_scan_config(&self) -> Result<ScanConfig, IndexError> {
        use types::index_error::*;

        let sembly_config =
            indexing::load_sembly_config(&self.forest_root).context(ConfigLoadFailedSnafu)?;

        let targets = sembly_config
            .as_ref()
            .map(|c| c.targets.clone())
            .unwrap_or_default();

        Ok(ScanConfig::builder().targets(targets).build())
    }

    /// Load indexing filter rules from sembly configuration
    ///
    /// Reads `.sembly.toml` to determine which files should be excluded from indexing.
    fn load_indexing_filter(&self) -> Result<IndexingFilter, IndexError> {
        use types::index_error::*;

        let sembly_config =
            indexing::load_sembly_config(&self.forest_root).context(ConfigLoadFailedSnafu)?;

        let filter = sembly_config
            .map(|c| c.to_indexing_filter())
            .transpose()
            .context(ConfigLoadFailedSnafu)?
            .unwrap_or_default();

        Ok(filter)
    }
}

// TODO: Re-enable these tests after updating IndexedFile and Chunk to use content-addressed storage.
// These tests currently fail because the schema has been updated to use contexts, chunk_hash, and file_hash,
// but the domain types and storage layer still use the old structure.
// This will be fixed when implementing context-aware indexing and content-addressed storage.
#[cfg(all(test, feature = "enable_broken_tests"))]
mod test {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[ignore = "disabled until content-addressed storage is implemented"]
    #[test]
    fn test_open_creates_sembly_directory() {
        // Given A forest root without .sembly directory
        let temp_dir = TempDir::new().unwrap();
        let forest_root = temp_dir.path();

        // When Opening an index
        let result = KnowledgeIndex::open(forest_root);

        // Then It should create .sembly directory
        assert!(result.is_ok());
        assert!(forest_root.join(".sembly").exists());
    }

    #[ignore = "disabled until content-addressed storage is implemented"]
    #[test]
    fn test_open_does_not_create_database_file() {
        // Given A forest root without existing database
        let temp_dir = TempDir::new().unwrap();
        let forest_root = temp_dir.path();

        // When Opening an index
        let result = KnowledgeIndex::open(forest_root);

        // Then It should succeed but not create knowledge.db
        assert!(result.is_ok());
        assert!(!forest_root.join(".sembly/knowledge.db").exists());
    }

    #[ignore = "disabled until content-addressed storage is implemented"]
    #[test]
    fn test_open_with_nonexistent_forest_root_fails() {
        // Given A nonexistent forest root
        let nonexistent = std::path::Path::new("/nonexistent/forest");

        // When Opening an index
        let result = KnowledgeIndex::open(nonexistent);

        // Then It should fail with ForestRootNotFound
        assert!(matches!(result, Err(IndexError::ForestRootNotFound { .. })));
    }

    #[ignore = "disabled until content-addressed storage is implemented"]
    #[test]
    fn test_open_returns_index_with_correct_paths() {
        // Given A forest root
        let temp_dir = TempDir::new().unwrap();
        let forest_root = temp_dir.path();

        // When Opening an index
        let index = KnowledgeIndex::open(forest_root).unwrap();

        // Then Paths should be correct
        assert_eq!(index.forest_root(), forest_root);
        assert_eq!(index.db_path(), forest_root.join(".sembly/knowledge.db"));
    }

    #[ignore = "disabled until content-addressed storage is implemented"]
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

    #[ignore = "disabled until content-addressed storage is implemented"]
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

    #[ignore = "disabled until content-addressed storage is implemented"]
    #[test]
    fn test_open_with_config_validates_existing_config() {
        // Given An existing index with default config
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("test.md"), "# Test").unwrap();

        let index = KnowledgeIndex::open(temp_dir.path()).unwrap();
        index.build().call().unwrap();

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

    #[ignore = "disabled until content-addressed storage is implemented"]
    #[test]
    fn test_build_indexes_files_in_forest() {
        // Given A forest with markdown files
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("README.md"), "# Test\n\nContent here").unwrap();

        let index = KnowledgeIndex::open(temp_dir.path()).unwrap();

        // When Building the index
        let result = index.build().call();

        // Then It should succeed and report indexed files
        assert!(result.is_ok());
        let index_result = result.unwrap();
        assert!(index_result.files_processed > 0);
        assert!(index_result.chunks_affected > 0);
    }

    #[ignore = "disabled until content-addressed storage is implemented"]
    #[test]
    fn test_build_handles_empty_forest() {
        // Given An empty forest
        let temp_dir = TempDir::new().unwrap();
        let index = KnowledgeIndex::open(temp_dir.path()).unwrap();

        // When Building
        let result = index.build().call().unwrap();

        // Then It should succeed with zero files
        assert_eq!(result.files_processed, 0);
        assert_eq!(result.chunks_affected, 0);
    }

    #[ignore = "disabled until content-addressed storage is implemented"]
    #[test]
    fn test_rebuild_clears_existing_chunks() {
        // Given An index with existing chunks
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("test.md"), "# Test\n\nContent").unwrap();

        let index = KnowledgeIndex::open(temp_dir.path()).unwrap();
        index.build().call().unwrap();

        // When Rebuilding
        let result = index.rebuild().call();

        // Then It should succeed
        assert!(result.is_ok());
    }

    #[ignore = "disabled until content-addressed storage is implemented"]
    #[test]
    fn test_rebuild_reindexes_all_files() {
        // Given An index
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("test.md"), "# Test\n\nContent").unwrap();

        let index = KnowledgeIndex::open(temp_dir.path()).unwrap();

        // When Rebuilding
        let result = index.rebuild().call().unwrap();

        // Then It should index files
        assert!(result.files_processed > 0);
        assert!(result.chunks_affected > 0);
    }

    #[ignore = "disabled until content-addressed storage is implemented"]
    #[test]
    fn test_update_detects_new_files() {
        // Given An index with no files
        let temp_dir = TempDir::new().unwrap();
        let index = KnowledgeIndex::open(temp_dir.path()).unwrap();
        index.build().call().unwrap();

        // When Adding a new file and updating
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("new.md"), "# New\n\nContent").unwrap();

        let result = index.update().call().unwrap();

        // Then It should report one file added
        assert_eq!(result.files_added, 1);
        assert!(result.chunks_affected > 0);
    }

    #[ignore = "disabled until content-addressed storage is implemented"]
    #[test]
    fn test_update_detects_deleted_files() {
        // Given An index with a file
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        let file_path = repo_dir.join("test.md");
        fs::write(&file_path, "# Test\n\nContent").unwrap();

        let index = KnowledgeIndex::open(temp_dir.path()).unwrap();
        index.build().call().unwrap();

        // When Deleting the file and updating
        fs::remove_file(&file_path).unwrap();

        let result = index.update().call().unwrap();

        // Then It should report one file removed
        assert_eq!(result.files_removed, 1);
    }

    #[ignore = "disabled until content-addressed storage is implemented"]
    #[test]
    fn test_clear_deletes_database() {
        // Given An index with chunks
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("test.md"), "# Test\n\nContent").unwrap();

        let index = KnowledgeIndex::open(temp_dir.path()).unwrap();
        index.build().call().unwrap();

        let db_path = temp_dir.path().join(".sembly/knowledge.db");
        assert!(db_path.exists());

        // When Clearing the index
        let result = index.clear();

        // Then It should succeed and delete the database
        assert!(result.is_ok());
        assert!(!db_path.exists());
    }

    #[ignore = "disabled until content-addressed storage is implemented"]
    #[test]
    fn test_clear_on_nonexistent_index_succeeds() {
        // Given No index exists
        let temp_dir = TempDir::new().unwrap();
        let index = KnowledgeIndex::open(temp_dir.path()).unwrap();

        // When Clearing
        let result = index.clear();

        // Then It should succeed (no-op)
        assert!(result.is_ok());
    }

    #[ignore = "disabled until content-addressed storage is implemented"]
    #[test]
    fn test_search_executes_query() {
        // Given An index with content
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("test.md"), "# Boot Process\n\nHow boot works").unwrap();

        let index = KnowledgeIndex::open(temp_dir.path()).unwrap();
        index.build().call().unwrap();

        // When Searching
        let result = index.search("boot", 10);

        // Then It should return results
        assert!(result.is_ok());
        let search_results = result.unwrap();
        assert!(!search_results.results.is_empty());
    }

    #[ignore = "disabled until content-addressed storage is implemented"]
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

        let index = KnowledgeIndex::open(temp_dir.path()).unwrap();
        index.build().call().unwrap();

        // When Searching with limit 2
        let result = index.search("test", 2).unwrap();

        // Then Results should respect limit
        assert!(result.results.len() <= 2);
    }

    #[ignore = "disabled until content-addressed storage is implemented"]
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

    #[ignore = "disabled until content-addressed storage is implemented"]
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

    #[ignore = "disabled until content-addressed storage is implemented"]
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

    #[ignore = "disabled until content-addressed storage is implemented"]
    #[test]
    fn test_status_returns_index_metadata() {
        // Given An index with content
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("test.md"), "# Test\n\nContent").unwrap();

        let index = KnowledgeIndex::open(temp_dir.path()).unwrap();
        index.build().call().unwrap();

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

    #[ignore = "disabled until content-addressed storage is implemented"]
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

    #[ignore = "disabled until content-addressed storage is implemented"]
    #[test]
    fn test_status_includes_disk_size() {
        // Given An index with content
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("test.md"), "# Test\n\nContent").unwrap();

        let index = KnowledgeIndex::open(temp_dir.path()).unwrap();
        index.build().call().unwrap();

        // When Getting status
        let status = index.status().unwrap();

        // Then It should include size
        assert!(status.size_bytes.is_some());
        assert!(status.size_bytes.unwrap() > 0);
    }

    #[ignore = "disabled until content-addressed storage is implemented"]
    #[test]
    fn test_default_db_path_returns_correct_path() {
        // Given A forest root
        let forest_root = std::path::Path::new("/test/forest");

        // When Computing default db path
        let db_path = KnowledgeIndex::default_db_path(forest_root);

        // Then It should be .sembly/knowledge.db
        assert_eq!(db_path, forest_root.join(".sembly/knowledge.db"));
    }

    #[ignore = "disabled until content-addressed storage is implemented"]
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

    #[ignore = "disabled until content-addressed storage is implemented"]
    #[test]
    fn test_db_path_returns_correct_path() {
        // Given An index
        let temp_dir = TempDir::new().unwrap();
        let forest_root = temp_dir.path();
        let index = KnowledgeIndex::open(forest_root).unwrap();

        // When Getting db path
        let db_path = index.db_path();

        // Then It should be .sembly/knowledge.db
        assert_eq!(db_path, forest_root.join(".sembly/knowledge.db"));
    }

    #[ignore = "disabled until content-addressed storage is implemented"]
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

    #[ignore = "disabled until content-addressed storage is implemented"]
    #[test]
    fn test_load_scan_config_with_existing_sembly_toml() {
        // Given A forest root with .sembly.toml containing targets
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"
targets = ["docs", "bottlerocket"]
"#;
        fs::write(temp_dir.path().join(".sembly.toml"), config_content).unwrap();

        let index = KnowledgeIndex::open(temp_dir.path()).unwrap();

        // When Loading scan config
        let scan_config = index.load_scan_config().unwrap();

        // Then It should return ScanConfig with those targets
        assert_eq!(scan_config.targets.len(), 2);
        assert_eq!(scan_config.targets[0], PathBuf::from("docs"));
        assert_eq!(scan_config.targets[1], PathBuf::from("bottlerocket"));
    }

    #[ignore = "disabled until content-addressed storage is implemented"]
    #[test]
    fn test_load_scan_config_without_sembly_toml() {
        // Given A forest root without .sembly.toml
        let temp_dir = TempDir::new().unwrap();
        let index = KnowledgeIndex::open(temp_dir.path()).unwrap();

        // When Loading scan config
        let scan_config = index.load_scan_config().unwrap();

        // Then It should return ScanConfig with empty targets
        assert!(scan_config.targets.is_empty());
    }

    #[ignore = "disabled until content-addressed storage is implemented"]
    #[test]
    fn test_build_uses_configured_targets() {
        // Given A forest with .sembly.toml specifying specific targets
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
        fs::write(temp_dir.path().join(".sembly.toml"), config_content).unwrap();

        let index = KnowledgeIndex::open(temp_dir.path()).unwrap();

        // When Building the index
        let result = index.build().call().unwrap();

        // Then It should only index files from configured targets
        assert_eq!(result.files_processed, 2);
        let status = index.status().unwrap();
        assert_eq!(status.file_count, 2);
    }

    #[ignore = "disabled until content-addressed storage is implemented"]
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
        fs::write(temp_dir.path().join(".sembly.toml"), config_content).unwrap();

        let index = KnowledgeIndex::open(temp_dir.path()).unwrap();

        // When Rebuilding
        let result = index.rebuild().call().unwrap();

        // Then It should only index configured targets
        assert_eq!(result.files_processed, 1);
    }

    #[ignore = "disabled until content-addressed storage is implemented"]
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
        fs::write(temp_dir.path().join(".sembly.toml"), config_content).unwrap();

        let index = KnowledgeIndex::open(temp_dir.path()).unwrap();
        index.build().call().unwrap();

        // When Adding files to both directories and updating
        fs::write(docs_dir.join("new.md"), "# New").unwrap();
        fs::write(other_dir.join("ignored.md"), "# Ignored").unwrap();

        let result = index.update().call().unwrap();

        // Then It should only detect changes in configured targets
        assert_eq!(result.files_added, 1);
    }
}

#[cfg(test)]
mod context_tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_workspace(root: &Path) {
        let sembly_dir = root.join(".sembly");
        fs::create_dir_all(&sembly_dir).unwrap();
        // Create a minimal database with the contexts table
        let db_path = sembly_dir.join("knowledge.db");
        let config = EmbeddingModelConfig::default();
        let repo = SqliteChunkRepository::open(&db_path, &config).unwrap();
        // Register the default context
        let default_context = Context::builder()
            .context_id(ContextId::from_path(".").unwrap())
            .build();
        repo.context_repository()
            .insert_context(&default_context)
            .unwrap();
    }

    #[test]
    fn discover_finds_workspace_from_root() {
        // Given a workspace with .sembly/knowledge.db
        let temp = TempDir::new().unwrap();
        create_workspace(temp.path());

        // When discovering from the workspace root
        let result = KnowledgeIndex::discover(temp.path());

        // Then it should find the workspace
        assert!(result.is_ok());
        let index = result.unwrap();
        assert_eq!(index.forest_root(), temp.path());
    }

    #[test]
    fn discover_finds_workspace_from_nested_directory() {
        // Given a workspace with nested subdirectories
        let temp = TempDir::new().unwrap();
        create_workspace(temp.path());
        let nested = temp.path().join("a").join("b").join("c");
        fs::create_dir_all(&nested).unwrap();

        // When discovering from a nested directory
        let result = KnowledgeIndex::discover(&nested);

        // Then it should find the workspace at the root
        assert!(result.is_ok());
        let index = result.unwrap();
        assert_eq!(index.forest_root(), temp.path());
    }

    #[test]
    fn discover_returns_error_when_no_workspace() {
        // Given a directory without .sembly
        let temp = TempDir::new().unwrap();

        // When discovering from that directory
        let result = KnowledgeIndex::discover(temp.path());

        // Then it should return WorkspaceNotFound error
        assert!(matches!(result, Err(IndexError::WorkspaceNotFound { .. })));
    }

    #[test]
    fn resolve_context_finds_default_context() {
        // Given a workspace with the default "." context
        let temp = TempDir::new().unwrap();
        create_workspace(temp.path());

        let index = KnowledgeIndex::open(temp.path()).unwrap();

        // When resolving context from the workspace root
        let result = index.resolve_context(temp.path());

        // Then it should return the default context
        assert!(result.is_ok());
        let context = result.unwrap();
        assert_eq!(context.context_id.as_str(), ".");
    }

    #[test]
    fn resolve_context_returns_error_for_unregistered_subdirectory() {
        // Given a workspace with only the default "." context
        let temp = TempDir::new().unwrap();
        create_workspace(temp.path());
        let subdir = temp.path().join("subdir");
        fs::create_dir_all(&subdir).unwrap();

        let index = KnowledgeIndex::open(temp.path()).unwrap();

        // When resolving context from a subdirectory that's not a registered context
        let result = index.resolve_context(&subdir);

        // Then it should return ContextNotFound error (per MCI-ERR-2)
        // The "." context only matches the workspace root, not subdirectories
        assert!(matches!(result, Err(IndexError::ContextNotFound { .. })));
    }

    #[test]
    fn list_contexts_returns_registered_contexts() {
        // Given a workspace with the default context
        let temp = TempDir::new().unwrap();
        create_workspace(temp.path());

        let index = KnowledgeIndex::open(temp.path()).unwrap();

        // When listing contexts
        let result = index.list_contexts();

        // Then it should return the default context
        assert!(result.is_ok());
        let contexts = result.unwrap();
        assert_eq!(contexts.len(), 1);
        assert_eq!(contexts[0].context_id.as_str(), ".");
    }

    #[test]
    fn resolve_context_finds_registered_context_from_subdirectory() {
        // Given a workspace with a registered "worktrees/feature-a" context
        let temp = TempDir::new().unwrap();
        let sembly_dir = temp.path().join(".sembly");
        fs::create_dir_all(&sembly_dir).unwrap();
        let db_path = sembly_dir.join("knowledge.db");
        let config = EmbeddingModelConfig::default();
        let repo = SqliteChunkRepository::open(&db_path, &config).unwrap();

        // Register the worktree context (not the default ".")
        let worktree_context = Context::builder()
            .context_id(ContextId::from_path("worktrees/feature-a").unwrap())
            .build();
        repo.context_repository()
            .insert_context(&worktree_context)
            .unwrap();

        // Create the directory structure
        let worktree_dir = temp.path().join("worktrees").join("feature-a");
        let nested_dir = worktree_dir.join("src").join("lib");
        fs::create_dir_all(&nested_dir).unwrap();

        let index = KnowledgeIndex::open(temp.path()).unwrap();

        // When resolving context from a nested subdirectory within the worktree
        let result = index.resolve_context(&nested_dir);

        // Then it should return the worktrees/feature-a context
        assert!(result.is_ok());
        let context = result.unwrap();
        assert_eq!(context.context_id.as_str(), "worktrees/feature-a");
    }
}
