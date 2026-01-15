//! Build and rebuild operations for the knowledge index.

use std::sync::Arc;

use snafu::ResultExt;

use crate::knowledge::domain::{BatchSize, Context, ContextId, IndexMetadata};
use crate::knowledge::facade::KnowledgeIndex;
use crate::knowledge::facade::types::IndexError;
use crate::knowledge::indexing::{
    BatchConfig, IndexDataProvider, IndexResult, IndexStrategy, Indexer, ProgressReporter,
};
use crate::knowledge::storage::sqlite::SqliteChunkRepository;
use crate::knowledge::storage::{ChunkRepository, ContextRepository};

pub(in crate::knowledge::facade) fn build(
    index: &KnowledgeIndex,
    progress: Option<Arc<dyn ProgressReporter>>,
    batch_size: BatchSize,
    context_id: ContextId,
) -> Result<IndexResult, IndexError> {
    use crate::knowledge::facade::types::index_error::*;

    snafu::ensure!(
        !index.db_path.exists(),
        IndexAlreadyExistsSnafu {
            path: index.db_path.display().to_string()
        }
    );

    let mut repository = SqliteChunkRepository::open(&index.db_path, &index.config)
        .context(DatabaseAccessFailedSnafu)?;

    let metadata = IndexMetadata::builder()
        .last_build(std::time::SystemTime::now())
        .chunk_count(0)
        .file_count(0)
        .model_config(index.config.clone())
        .build();
    repository
        .set_metadata(&metadata)
        .context(DatabaseAccessFailedSnafu)?;

    let context = Context::builder().context_id(context_id.clone()).build();
    repository
        .context_repository()
        .insert_context(&context)
        .context(ContextRegistrationFailedSnafu)?;

    let provider = Box::new(super::create_provider(index)?) as Box<dyn IndexDataProvider>;
    let scan_config = super::load_scan_config_for_context(index, &context_id)?;
    let filter = super::load_indexing_filter(index)?;
    let repository = super::repository(index)?;

    let batch_config = BatchConfig { batch_size };

    let mut indexer = Indexer::builder()
        .index_root(&index.index_root)
        .repository(repository)
        .config(&index.config)
        .provider(provider)
        .scan_config(scan_config)
        .filter(filter)
        .maybe_progress(progress)
        .batch_config(batch_config)
        .context_id(context_id)
        .build()
        .context(IndexingFailedSnafu)?;

    let result = indexer
        .index(IndexStrategy::Build)
        .context(IndexingFailedSnafu)?;

    super::update_last_build_timestamp(index)?;

    Ok(result)
}

pub(in crate::knowledge::facade) fn rebuild(
    index: &KnowledgeIndex,
    progress: Option<Arc<dyn ProgressReporter>>,
    batch_size: BatchSize,
    context_id: ContextId,
) -> Result<IndexResult, IndexError> {
    use crate::knowledge::facade::types::index_error::*;

    let preserved_contexts = if index.db_path.exists() {
        let old_repo = SqliteChunkRepository::open(&index.db_path, &index.config).ok();
        let contexts = old_repo
            .as_ref()
            .and_then(|r| r.context_repository().list_contexts().ok())
            .unwrap_or_default();
        std::fs::remove_file(&index.db_path).context(IndexDeletionFailedSnafu)?;
        contexts
    } else {
        Vec::new()
    };

    let mut repository = SqliteChunkRepository::open(&index.db_path, &index.config)
        .context(DatabaseAccessFailedSnafu)?;

    let metadata = IndexMetadata::builder()
        .last_build(std::time::SystemTime::now())
        .chunk_count(0)
        .file_count(0)
        .model_config(index.config.clone())
        .build();
    repository
        .set_metadata(&metadata)
        .context(DatabaseAccessFailedSnafu)?;

    let ctx_repo = repository.context_repository();
    let preserved_ids: std::collections::HashSet<_> =
        preserved_contexts.iter().map(|c| &c.context_id).collect();

    for ctx in &preserved_contexts {
        ctx_repo
            .insert_context(ctx)
            .context(ContextRegistrationFailedSnafu)?;
    }

    #[expect(clippy::expect_used)]
    let default_id = ContextId::from_path(".").expect("'.' is valid context id");
    if !preserved_ids.contains(&default_id) {
        let default_context = Context::builder().context_id(default_id).build();
        ctx_repo
            .insert_context(&default_context)
            .context(ContextRegistrationFailedSnafu)?;
    }

    if context_id.as_str() != "." && !preserved_ids.contains(&context_id) {
        let context = Context::builder().context_id(context_id.clone()).build();
        ctx_repo
            .insert_context(&context)
            .context(ContextRegistrationFailedSnafu)?;
    }

    let provider = Box::new(super::create_provider(index)?) as Box<dyn IndexDataProvider>;
    let scan_config = super::load_scan_config_for_context(index, &context_id)?;
    let filter = super::load_indexing_filter(index)?;
    let repository = super::repository(index)?;

    let batch_config = BatchConfig { batch_size };

    let mut indexer = Indexer::builder()
        .index_root(&index.index_root)
        .repository(repository)
        .config(&index.config)
        .provider(provider)
        .scan_config(scan_config)
        .filter(filter)
        .maybe_progress(progress)
        .batch_config(batch_config)
        .context_id(context_id)
        .build()
        .context(IndexingFailedSnafu)?;

    let result = indexer
        .index(IndexStrategy::Build)
        .context(IndexingFailedSnafu)?;

    super::update_last_build_timestamp(index)?;

    Ok(result)
}

#[cfg(test)]
mod test {
    use crate::knowledge::KnowledgeIndex;
    use crate::knowledge::facade::test_helpers::test_helpers::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_build_indexes_files_in_forest() {
        // Given a forest with a markdown file
        let temp_dir = TempDir::new().unwrap();
        create_test_file(
            temp_dir.path(),
            "test-repo",
            "README.md",
            "# Test\n\nContent here",
        );

        // When building the index
        let index = KnowledgeIndex::open(temp_dir.path()).unwrap();
        let result = index.build().call();

        // Then files and chunks are indexed
        assert!(result.is_ok());
        let index_result = result.unwrap();
        assert!(index_result.files_processed > 0);
        assert!(index_result.chunks_affected > 0);
    }

    #[test]
    fn test_build_handles_empty_forest() {
        // Given an empty forest
        let temp_dir = TempDir::new().unwrap();
        let index = KnowledgeIndex::open(temp_dir.path()).unwrap();

        // When building the index
        let result = index.build().call().unwrap();

        // Then no files or chunks are processed
        assert_eq!(result.files_processed, 0);
        assert_eq!(result.chunks_affected, 0);
    }

    #[test]
    fn test_build_uses_configured_targets() {
        // Given a forest with configured targets excluding some directories
        let temp_dir = TempDir::new().unwrap();
        fs::create_dir(temp_dir.path().join(".git")).unwrap();
        create_test_file(
            temp_dir.path(),
            "docs",
            "guide.md",
            "# Guide\n\nDocumentation content",
        );
        create_test_file(
            temp_dir.path(),
            "bottlerocket",
            "README.md",
            "# Bottlerocket\n\nProject info",
        );
        create_test_file(
            temp_dir.path(),
            "ignored",
            "secret.md",
            "# Secret\n\nSecret content",
        );

        let config_content = r#"
targets = ["docs", "bottlerocket"]
"#;
        fs::write(temp_dir.path().join("crumbly.toml"), config_content).unwrap();

        // When building the index
        let index = KnowledgeIndex::open(temp_dir.path()).unwrap();
        let result = index.build().call().unwrap();

        // Then only files in configured targets are indexed
        assert_eq!(result.files_processed, 2);
        let status = index.status().unwrap();
        assert_eq!(status.file_count, 2);
    }

    #[test]
    fn test_rebuild_clears_existing_chunks() {
        // Given a forest with an existing index
        let temp_dir = TempDir::new().unwrap();
        let _index = test_index_with_content(&temp_dir);

        // When rebuilding the index
        let index = KnowledgeIndex::open(temp_dir.path()).unwrap();
        let result = index.rebuild().call();

        // Then rebuild succeeds
        assert!(result.is_ok());
    }

    #[test]
    fn test_rebuild_reindexes_all_files() {
        // Given a forest with a markdown file
        let temp_dir = TempDir::new().unwrap();
        create_test_file(temp_dir.path(), "test-repo", "test.md", "# Test\n\nContent");

        // When rebuilding the index
        let index = KnowledgeIndex::open(temp_dir.path()).unwrap();
        let result = index.rebuild().call().unwrap();

        // Then files and chunks are reindexed
        assert!(result.files_processed > 0);
        assert!(result.chunks_affected > 0);
    }

    #[test]
    fn test_rebuild_uses_configured_targets() {
        // Given a forest with configured targets
        let temp_dir = TempDir::new().unwrap();
        fs::create_dir(temp_dir.path().join(".git")).unwrap();
        create_test_file(
            temp_dir.path(),
            "docs",
            "guide.md",
            "# Guide\n\nDocumentation",
        );
        create_test_file(
            temp_dir.path(),
            "other",
            "other.md",
            "# Other\n\nOther content",
        );

        let config_content = r#"
targets = ["docs"]
"#;
        fs::write(temp_dir.path().join("crumbly.toml"), config_content).unwrap();

        // When rebuilding the index
        let index = KnowledgeIndex::open(temp_dir.path()).unwrap();
        let result = index.rebuild().call().unwrap();

        // Then only files in configured targets are indexed
        assert_eq!(result.files_processed, 1);
    }
}
