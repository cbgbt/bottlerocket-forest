//! Core indexing operations

use snafu::ResultExt;

use super::super::{IndexDataProvider, IndexableFile};
use super::types::IndexingError;
use crate::knowledge::chunking::{ChunkingDispatcher, ChunkingError, ChunkingInput, DispatchError};
use crate::knowledge::domain::{
    Chunk, ChunkSource, ChunkableContent, IndexedChunk, LineCount, LineNumber, LineRange, Timestamp,
};

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
) -> Result<Vec<IndexedChunk>, Result<(), IndexingError>> {
    match process_file(file, dispatcher, provider) {
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
            .line_range(
                LineRange::builder()
                    .start(LineNumber::try_new(1).expect("1 is valid line number"))
                    .line_count(LineCount::try_new(1).expect("1 is valid line count"))
                    .build(),
            )
            .build(),
    };

    let chunks = match dispatcher.chunk_file(&input) {
        Some(result) => result.context(ChunkingFailedSnafu)?,
        None => return Ok(vec![]),
    };

    index_chunks(chunks, provider)
}

/// Index multiple chunks, using batch operations when available
fn index_chunks(
    chunks: Vec<Chunk>,
    provider: &dyn IndexDataProvider,
) -> Result<Vec<IndexedChunk>, IndexingError> {
    use super::types::indexing_error::*;

    if chunks.is_empty() {
        return Ok(vec![]);
    }

    let texts: Vec<_> = chunks.iter().map(|c| c.content.text.as_ref()).collect();

    let index_data_list = provider
        .generate_batch(&texts)
        .context(IndexDataGenerationFailedSnafu)?;

    let timestamp = Timestamp::now();

    Ok(chunks
        .into_iter()
        .zip(index_data_list)
        .map(|(chunk, index_data)| {
            IndexedChunk::builder()
                .chunk(chunk)
                .index_data(index_data)
                .indexed_at(timestamp)
                .build()
        })
        .collect())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::knowledge::domain::{
        ChunkContent, ChunkContext, ChunkId, ChunkSource, ForestRelativePath, MarkdownContext,
        RepoName, TokenCount,
    };

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
}
