//! Core file processing operations for indexing
//!
//! Provides functions for processing individual files during indexing: reading
//! content, chunking via the dispatcher, generating embeddings, and handling
//! parse errors gracefully.

use snafu::ResultExt;

use super::super::{IndexDataProvider, IndexableFile, ProgressReporter};
use super::types::IndexingError;
use crate::knowledge::chunking::{ChunkingDispatcher, ChunkingError, ChunkingInput, DispatchError};
use crate::knowledge::domain::{Chunk, ChunkSource, ChunkableContent, IndexedChunk, Timestamp};

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

    let input = ChunkingInput {
        content: ChunkableContent::new(content),
        source: ChunkSource::builder()
            .file_path(file.relative_path.clone())
            .repo_name(file.repo_name.clone())
            .build(),
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::knowledge::indexing::ScanError;
    use crate::knowledge::storage::StorageError;

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
}
