//! Core indexing operations

use crate::knowledge::chunking::{ChunkingDispatcher, ChunkingInput};
use crate::knowledge::domain::{
    Chunk, ChunkSource, ChunkableContent, Embedding, IndexData, IndexMode, IndexedChunk, LineCount,
    LineNumber, LineRange, Timestamp,
};

use super::super::IndexableFile;
use super::types::IndexError;

use snafu::ResultExt;

/// Convert a domain chunk to an indexed chunk
///
/// Wraps the chunk with index data (BM25 terms or embedding) and timestamp.
pub(super) fn index_chunk(chunk: Chunk, mode: IndexMode) -> IndexedChunk {
    let index_data = match mode {
        IndexMode::Fast => {
            let bm25_terms =
                crate::knowledge::storage::bm25::calculate_bm25_terms(chunk.content.text.as_ref());
            IndexData::Fast { bm25_terms }
        }
        IndexMode::Best => {
            // TODO: Replace with real EmbeddingProvider when implemented (Phase 6, Commit 21-22)
            let embedding =
                Embedding::try_new(vec![0.0; 384]).expect("placeholder embedding should be valid");
            IndexData::Best { embedding }
        }
    };

    IndexedChunk::builder()
        .chunk(chunk)
        .index_data(index_data)
        .indexed_at(Timestamp::now())
        .build()
}

/// Process a single file: read, chunk, and index
///
/// Reads the file content, chunks it using the dispatcher, and wraps each chunk
/// with index data.
pub(super) fn process_file(
    file: &IndexableFile,
    dispatcher: &ChunkingDispatcher,
    mode: IndexMode,
) -> Result<Vec<IndexedChunk>, IndexError> {
    use super::types::index_error::*;

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

    let indexed_chunks = chunks
        .into_iter()
        .map(|chunk| index_chunk(chunk, mode))
        .collect();

    Ok(indexed_chunks)
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

    #[test]
    fn test_index_chunk_adds_timestamp() {
        // Given A chunk
        let chunk = create_test_chunk("test.md", "test content");
        let before = Timestamp::now();

        // When Indexing the chunk
        let indexed = index_chunk(chunk, IndexMode::Fast);

        // Then It should have a timestamp
        let after = Timestamp::now();
        assert!(indexed.indexed_at >= before);
        assert!(indexed.indexed_at <= after);
    }

    #[test]
    fn test_index_chunk_adds_fast_mode_data() {
        // Given A chunk with content
        let chunk = create_test_chunk("test.md", "rust programming language");

        // When Indexing in Fast mode
        let indexed = index_chunk(chunk, IndexMode::Fast);

        // Then It should have BM25 terms
        assert!(matches!(indexed.index_data, IndexData::Fast { .. }));
        if let IndexData::Fast { ref bm25_terms } = indexed.index_data {
            assert!(bm25_terms.contains_key("rust"));
            assert!(bm25_terms.contains_key("programming"));
        }
    }

    #[test]
    fn test_index_chunk_adds_best_mode_data() {
        // Given A chunk
        let chunk = create_test_chunk("test.md", "test content");

        // When Indexing in Best mode
        let indexed = index_chunk(chunk, IndexMode::Best);

        // Then It should have an embedding
        assert!(matches!(indexed.index_data, IndexData::Best { .. }));
        if let IndexData::Best { ref embedding } = indexed.index_data {
            assert_eq!(embedding.as_slice().len(), 384);
        }
    }
}
