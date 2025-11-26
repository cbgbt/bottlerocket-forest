//! Core file processing operations for indexing
//!
//! Provides functions for processing individual files during indexing: reading
//! content, chunking via the dispatcher, generating embeddings, and handling
//! parse errors gracefully.

use snafu::ResultExt;

use super::super::{IndexDataProvider, IndexableFile, ProgressReporter};
use super::types::IndexingError;
use crate::knowledge::chunking::{ChunkingDispatcher, ChunkingError, ChunkingInput, DispatchError};
use crate::knowledge::domain::{
    Chunk, ChunkHash, ChunkSource, ChunkableContent, FileHash, IndexedChunk, Timestamp,
};
use crate::knowledge::storage::ChunkRepository;

/// Process a file with graceful error handling for parse failures
///
/// Returns Ok with chunks on success, Err(Ok(())) if the file should be skipped
/// due to a parse error, or Err(Err(error)) for fatal errors that should be
/// propagated to the caller.
pub(super) fn process_file_gracefully(
    file: &IndexableFile,
    dispatcher: &ChunkingDispatcher,
    provider: &dyn IndexDataProvider,
    progress: Option<&dyn ProgressReporter>,
) -> Result<Vec<IndexedChunk>, Result<(), IndexingError>> {
    match process_file(file, dispatcher, provider, progress) {
        Ok(chunks) => Ok(chunks),
        Err(e) => {
            if is_skippable_error(&e) {
                Err(Ok(()))
            } else {
                Err(Err(e))
            }
        }
    }
}

fn is_skippable_error(error: &IndexingError) -> bool {
    match error {
        // Skip files with parse errors
        IndexingError::ChunkingFailed {
            source:
                DispatchError::ChunkingFailed {
                    source: ChunkingError::ParseError { .. },
                },
        } => true,
        // Skip files with no matching strategy (shouldn't happen due to scanner filtering)
        IndexingError::ChunkingFailed {
            source: DispatchError::NoStrategyFound,
        } => true,
        // Strategy init failures are fatal - indicate configuration problem
        IndexingError::ChunkingFailed {
            source: DispatchError::StrategyInitFailed { .. },
        } => false,
        // Other chunking errors are fatal
        IndexingError::ChunkingFailed { .. } => false,
        // Skip files with read errors (permission denied, file deleted, etc.)
        IndexingError::ScanFailed { .. } => true,
        // Embedding errors are fatal - indicate systemic problem
        IndexingError::IndexDataGenerationFailed { .. } => false,
        // Storage errors are fatal
        IndexingError::StorageFailed { .. } => false,
    }
}

/// Process a single file by reading, chunking, and generating embeddings
pub(super) fn process_file(
    file: &IndexableFile,
    dispatcher: &ChunkingDispatcher,
    provider: &dyn IndexDataProvider,
    progress: Option<&dyn ProgressReporter>,
) -> Result<Vec<IndexedChunk>, IndexingError> {
    use super::types::indexing_error::*;

    let content = std::fs::read_to_string(file.absolute_path.to_string())
        .map_err(|e| super::super::ScanError::IoError {
            source: e,
            path: file.absolute_path.to_string(),
        })
        .context(ScanFailedSnafu)?;

    let file_hash = FileHash::from_reader(std::io::Cursor::new(content.as_bytes()))
        .map_err(|e| super::super::ScanError::IoError {
            source: e,
            path: file.absolute_path.to_string(),
        })
        .context(ScanFailedSnafu)?;

    let input = ChunkingInput {
        content: ChunkableContent::new(content),
        source: ChunkSource::builder()
            .file_path(file.relative_path.clone())
            .repo_name(file.repo_name.clone())
            .build(),
        file_hash,
    };

    let chunks = match dispatcher.chunk_file(&input) {
        Some(result) => result.context(ChunkingFailedSnafu)?,
        None => return Ok(vec![]),
    };

    index_chunks(chunks, provider, progress)
}

/// Generate embeddings for chunks and wrap them as IndexedChunks
fn index_chunks(
    chunks: Vec<Chunk>,
    provider: &dyn IndexDataProvider,
    progress: Option<&dyn ProgressReporter>,
) -> Result<Vec<IndexedChunk>, IndexingError> {
    use super::types::indexing_error::*;

    if chunks.is_empty() {
        return Ok(vec![]);
    }

    let texts: Vec<_> = chunks.iter().map(|c| c.content.text.as_ref()).collect();

    let index_data_list = provider
        .generate_batch_with_progress(&texts, progress)
        .context(IndexDataGenerationFailedSnafu)?;

    let timestamp = Timestamp::now();

    Ok(chunks
        .into_iter()
        .zip(index_data_list)
        .map(|(chunk, embedding)| {
            IndexedChunk::builder()
                .chunk(chunk)
                .embedding(embedding)
                .indexed_at(timestamp)
                .build()
        })
        .collect())
}

/// Generates embeddings for chunks, reusing existing embeddings when available.
///
/// Checks the repository for chunks that already have embeddings and skips
/// embedding generation for those chunks. Returns only newly indexed chunks
/// that need to be stored.
#[allow(dead_code)] // Will be used in future commits
pub(super) fn index_chunks_with_reuse<R: ChunkRepository>(
    chunks: Vec<Chunk>,
    repository: &R,
    provider: &dyn IndexDataProvider,
    progress: Option<&dyn ProgressReporter>,
) -> Result<Vec<IndexedChunk>, IndexingError> {
    let (chunks_needing_embeddings, reused_count) =
        filter_chunks_needing_embeddings(chunks, repository)?;

    let _ = (reused_count, chunks_needing_embeddings.len());
    // Logging would show: "Reusing {} existing embeddings, generating {} new"
    // but tracing is not available in sembly-core dependencies

    index_chunks(chunks_needing_embeddings, provider, progress)
}

/// Filters chunks to find those that need new embeddings.
///
/// Returns the chunks that don't already have embeddings in the repository.
#[allow(dead_code)] // Will be used in future commits
fn filter_chunks_needing_embeddings<R: ChunkRepository>(
    chunks: Vec<Chunk>,
    repository: &R,
) -> Result<(Vec<Chunk>, usize), IndexingError> {
    use super::types::indexing_error::*;

    let chunk_hashes: Vec<ChunkHash> = chunks.iter().map(|c| c.chunk_hash).collect();

    let existing = repository
        .has_embedding_batch(&chunk_hashes)
        .context(StorageFailedSnafu)?;

    let reused_count = existing.len();

    let chunks_needing_embeddings: Vec<Chunk> = chunks
        .into_iter()
        .filter(|c| !existing.contains(&c.chunk_hash))
        .collect();

    Ok((chunks_needing_embeddings, reused_count))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::knowledge::domain::{
        ChunkContent, ChunkContext, ChunkId, ChunkSource, Embedding, FileHash, ForestRelativePath,
        MarkdownContext, RepoName, TokenCount,
    };
    use crate::knowledge::indexing::ScanError;
    use crate::knowledge::indexing::provider::MockIndexDataProvider;
    use crate::knowledge::storage::StorageError;
    use crate::knowledge::storage::repository::MockChunkRepository;
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn make_test_chunk(text: &str) -> Chunk {
        let chunk_hash = ChunkHash::from_text(text);
        let file_hash = FileHash::new([0u8; 32]);

        Chunk::builder()
            .id(ChunkId::new(uuid::Uuid::new_v4()))
            .chunk_hash(chunk_hash)
            .file_hash(file_hash)
            .source(
                ChunkSource::builder()
                    .file_path(ForestRelativePath::try_new("test.md").unwrap())
                    .repo_name(RepoName::try_new("test-repo").unwrap())
                    .build(),
            )
            .content(
                ChunkContent::builder()
                    .text(text)
                    .token_count(TokenCount::try_new(10).unwrap())
                    .build(),
            )
            .context(ChunkContext::Markdown(
                MarkdownContext::builder().heading_hierarchy(vec![]).build(),
            ))
            .build()
    }

    #[test]
    fn test_is_skippable_error_parse_errors() {
        // Given A parse error
        let error = IndexingError::ChunkingFailed {
            source: DispatchError::ChunkingFailed {
                source: ChunkingError::ParseError {
                    file_path: "test.rs".to_string(),
                    source: Box::new(std::io::Error::other("parse failed")),
                },
            },
        };

        // When Checking if skippable
        let result = is_skippable_error(&error);

        // Then It should be skippable
        assert!(result);
    }

    #[test]
    fn test_is_skippable_error_scan_errors() {
        // Given A scan/IO error
        let error = IndexingError::ScanFailed {
            source: ScanError::IoError {
                path: "/test/file.md".to_string(),
                source: std::io::Error::other("permission denied"),
            },
        };

        // When Checking if skippable
        let result = is_skippable_error(&error);

        // Then It should be skippable
        assert!(result);
    }

    #[test]
    fn test_is_skippable_error_embedding_errors_not_skippable() {
        // Given An embedding generation error
        let error = IndexingError::IndexDataGenerationFailed {
            source: crate::knowledge::indexing::IndexDataError::EmbeddingFailed {
                source: Box::new(std::io::Error::other("model failed")),
            },
        };

        // When Checking if skippable
        let result = is_skippable_error(&error);

        // Then It should NOT be skippable
        assert!(!result);
    }

    #[test]
    fn test_is_skippable_error_storage_errors_not_skippable() {
        // Given A storage error
        let error = IndexingError::StorageFailed {
            source: StorageError::InvalidData {
                message: "corrupted data".to_string(),
            },
        };

        // When Checking if skippable
        let result = is_skippable_error(&error);

        // Then It should NOT be skippable
        assert!(!result);
    }

    // Tests for filter_chunks_needing_embeddings

    #[test]
    fn filter_chunks_needing_embeddings_returns_all_when_none_have_embeddings() {
        // Given chunks and a repository with no existing embeddings
        let chunks = vec![
            make_test_chunk("chunk one content"),
            make_test_chunk("chunk two content"),
            make_test_chunk("chunk three content"),
        ];

        let mut mock_repo = MockChunkRepository::new();
        mock_repo
            .expect_has_embedding_batch()
            .withf(move |hashes| hashes.len() == 3)
            .returning(|_| Ok(HashSet::new()));

        // When filtering chunks
        let (needs_embedding, reused_count) =
            filter_chunks_needing_embeddings(chunks.clone(), &mock_repo).unwrap();

        // Then all chunks should need embeddings
        assert_eq!(needs_embedding.len(), 3);
        assert_eq!(reused_count, 0);
        for (original, filtered) in chunks.iter().zip(needs_embedding.iter()) {
            assert_eq!(original.chunk_hash, filtered.chunk_hash);
        }
    }

    #[test]
    fn filter_chunks_needing_embeddings_returns_empty_when_all_have_embeddings() {
        // Given chunks that all have existing embeddings
        let chunks = vec![
            make_test_chunk("existing chunk one"),
            make_test_chunk("existing chunk two"),
        ];

        let mut mock_repo = MockChunkRepository::new();
        mock_repo
            .expect_has_embedding_batch()
            .returning(move |hashes| {
                // Return all hashes as having embeddings
                Ok(hashes.iter().copied().collect())
            });

        // When filtering chunks
        let (needs_embedding, reused_count) =
            filter_chunks_needing_embeddings(chunks.clone(), &mock_repo).unwrap();

        // Then no chunks should need embeddings
        assert!(needs_embedding.is_empty());
        assert_eq!(reused_count, 2);
    }

    #[test]
    fn filter_chunks_needing_embeddings_returns_only_chunks_without_embeddings() {
        // Given a mix of chunks with and without existing embeddings
        let chunk_with_embedding = make_test_chunk("has embedding");
        let chunk_without_embedding1 = make_test_chunk("needs embedding one");
        let chunk_without_embedding2 = make_test_chunk("needs embedding two");

        let existing_hash = chunk_with_embedding.chunk_hash;
        let chunks = vec![
            chunk_with_embedding,
            chunk_without_embedding1.clone(),
            chunk_without_embedding2.clone(),
        ];

        let mut mock_repo = MockChunkRepository::new();
        mock_repo.expect_has_embedding_batch().returning(move |_| {
            // Only the first chunk has an embedding
            let mut existing = HashSet::new();
            existing.insert(existing_hash);
            Ok(existing)
        });

        // When filtering chunks
        let (needs_embedding, reused_count) =
            filter_chunks_needing_embeddings(chunks, &mock_repo).unwrap();

        // Then only chunks without embeddings should be returned
        assert_eq!(needs_embedding.len(), 2);
        assert_eq!(reused_count, 1);
        assert!(
            needs_embedding
                .iter()
                .any(|c| c.chunk_hash == chunk_without_embedding1.chunk_hash)
        );
        assert!(
            needs_embedding
                .iter()
                .any(|c| c.chunk_hash == chunk_without_embedding2.chunk_hash)
        );
        assert!(
            !needs_embedding
                .iter()
                .any(|c| c.chunk_hash == existing_hash)
        );
    }

    // Tests for index_chunks_with_reuse

    #[test]
    fn index_chunks_with_reuse_generates_embeddings_only_for_new_chunks() {
        // Given chunks where some already have embeddings
        let new_chunk = make_test_chunk("new content needs embedding");
        let existing_chunk = make_test_chunk("existing content has embedding");
        let existing_hash = existing_chunk.chunk_hash;

        let chunks = vec![new_chunk.clone(), existing_chunk];

        let mut mock_repo = MockChunkRepository::new();
        mock_repo.expect_has_embedding_batch().returning(move |_| {
            let mut existing = HashSet::new();
            existing.insert(existing_hash);
            Ok(existing)
        });

        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();

        let mut mock_provider = MockIndexDataProvider::new();
        mock_provider
            .expect_generate_batch_with_progress()
            .returning(move |texts, _| {
                call_count_clone.fetch_add(texts.len(), Ordering::SeqCst);
                // Return embeddings for each text
                Ok(texts
                    .iter()
                    .map(|_| Embedding::try_new(vec![0.1; 384]).unwrap())
                    .collect())
            });

        // When indexing with reuse
        let result = index_chunks_with_reuse(chunks, &mock_repo, &mock_provider, None).unwrap();

        // Then embeddings should only be generated for new chunks
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
        // Result should contain the newly indexed chunk
        assert!(!result.is_empty());
    }

    #[test]
    fn index_chunks_with_reuse_does_not_call_provider_when_all_exist() {
        // Given chunks that all have existing embeddings
        let chunk1 = make_test_chunk("existing one");
        let chunk2 = make_test_chunk("existing two");
        let hash1 = chunk1.chunk_hash;
        let hash2 = chunk2.chunk_hash;

        let chunks = vec![chunk1, chunk2];

        let mut mock_repo = MockChunkRepository::new();
        mock_repo.expect_has_embedding_batch().returning(move |_| {
            let mut existing = HashSet::new();
            existing.insert(hash1);
            existing.insert(hash2);
            Ok(existing)
        });

        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();

        let mut mock_provider = MockIndexDataProvider::new();
        // This should never be called
        mock_provider
            .expect_generate_batch_with_progress()
            .returning(move |texts, _| {
                call_count_clone.fetch_add(1, Ordering::SeqCst);
                Ok(texts
                    .iter()
                    .map(|_| Embedding::try_new(vec![0.1; 384]).unwrap())
                    .collect())
            });

        // When indexing with reuse
        let result = index_chunks_with_reuse(chunks, &mock_repo, &mock_provider, None).unwrap();

        // Then provider should not be called at all
        assert_eq!(call_count.load(Ordering::SeqCst), 0);
        // Result should be empty since no new embeddings were generated
        assert!(result.is_empty());
    }

    #[test]
    fn index_chunks_with_reuse_returns_newly_indexed_chunks() {
        // Given all new chunks (none have existing embeddings)
        let chunk1 = make_test_chunk("brand new one");
        let chunk2 = make_test_chunk("brand new two");
        let chunks = vec![chunk1.clone(), chunk2.clone()];

        let mut mock_repo = MockChunkRepository::new();
        mock_repo
            .expect_has_embedding_batch()
            .returning(|_| Ok(HashSet::new()));

        let mut mock_provider = MockIndexDataProvider::new();
        mock_provider
            .expect_generate_batch_with_progress()
            .returning(|texts, _| {
                Ok(texts
                    .iter()
                    .map(|_| Embedding::try_new(vec![0.1; 384]).unwrap())
                    .collect())
            });

        // When indexing with reuse
        let result = index_chunks_with_reuse(chunks, &mock_repo, &mock_provider, None).unwrap();

        // Then all chunks should be returned as newly indexed
        assert_eq!(result.len(), 2);
        assert!(
            result
                .iter()
                .any(|ic| ic.chunk.chunk_hash == chunk1.chunk_hash)
        );
        assert!(
            result
                .iter()
                .any(|ic| ic.chunk.chunk_hash == chunk2.chunk_hash)
        );
    }
}
