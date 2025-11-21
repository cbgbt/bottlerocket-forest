//! Core indexing operations

use snafu::ResultExt;

use super::super::{IndexDataProvider, IndexableFile, ProgressReporter};
use super::types::IndexingError;
use crate::knowledge::chunking::{ChunkingDispatcher, ChunkingError, ChunkingInput, DispatchError};
use crate::knowledge::domain::{Chunk, ChunkSource, ChunkableContent, IndexedChunk, Timestamp};

/// Process a file with graceful error handling for parse failures
///
/// Returns:
/// - Ok: Successfully processed chunks
/// - Err(Ok): File was skipped due to parse error
/// - Err(Err): Fatal error - caller should propagate
pub(super) fn process_file_gracefully(
    file: &IndexableFile,
    dispatcher: &ChunkingDispatcher,
    provider: &dyn IndexDataProvider,
    progress: Option<&dyn ProgressReporter>,
) -> Result<Vec<IndexedChunk>, Result<(), IndexingError>> {
    match process_file(file, dispatcher, provider, progress) {
        Ok(chunks) => Ok(chunks),
        Err(e) => {
            if is_skippable_parse_error(&e) {
                Err(Ok(()))
            } else {
                Err(Err(e))
            }
        }
    }
}

fn is_skippable_parse_error(error: &IndexingError) -> bool {
    matches!(
        error,
        IndexingError::ChunkingFailed {
            source: DispatchError::ChunkingFailed {
                source: ChunkingError::ParseError { .. }
            }
        }
    )
}

/// Process a single file: read, chunk, and index
///
/// Reads the file content, chunks it using the dispatcher, and wraps each chunk
/// with index data.
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

/// Index multiple chunks, using batch operations when available
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
mod test {}
