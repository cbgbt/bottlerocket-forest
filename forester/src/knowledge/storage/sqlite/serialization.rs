//! Serialization helpers for converting between domain types and database formats

use snafu::ResultExt;

use crate::knowledge::domain::{
    Chunk, ChunkContent, ChunkContext, ChunkId, ChunkSource, Embedding, ForestRelativePath,
    LineCount, LineNumber, LineRange, MarkdownContext, RepoName, RustDocContext, TokenCount,
};
use crate::knowledge::storage::repository::{
    IndexData, IndexedChunk, StorageError, Timestamp, storage_error::*,
};

/// Parse a Chunk from a rusqlite::Row
///
/// Expects columns in order: id, file_path, repo_name, line_start, line_count,
/// context_type, context_data, content, token_count, last_modified, bm25_terms, index_mode, embedding
pub fn indexed_chunk_from_row(row: &rusqlite::Row) -> Result<IndexedChunk, StorageError> {
    let id_str: String = row.get(0).context(DatabaseSnafu)?;
    let file_path: String = row.get(1).context(DatabaseSnafu)?;
    let repo_name: String = row.get(2).context(DatabaseSnafu)?;
    let line_start: i64 = row.get(3).context(DatabaseSnafu)?;
    let line_count: i64 = row.get(4).context(DatabaseSnafu)?;
    let context_type: String = row.get(5).context(DatabaseSnafu)?;
    let context_data: String = row.get(6).context(DatabaseSnafu)?;
    let content: String = row.get(7).context(DatabaseSnafu)?;
    let token_count: i64 = row.get(8).context(DatabaseSnafu)?;
    let last_modified: i64 = row.get(9).context(DatabaseSnafu)?;
    let bm25_terms: Option<String> = row.get(10).context(DatabaseSnafu)?;
    let index_mode: String = row.get(11).context(DatabaseSnafu)?;
    let embedding_blob: Option<Vec<u8>> = row.get(12).context(DatabaseSnafu)?;

    let uuid = uuid::Uuid::parse_str(&id_str)
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync + 'static>)
        .context(InvalidFieldSnafu {
            field: "chunk_id".to_string(),
        })?;

    let context = deserialize_context(&context_type, &context_data)?;

    let index_data = match index_mode.as_str() {
        "fast" => {
            let terms_json = bm25_terms.ok_or_else(|| {
                InvalidDataSnafu {
                    message: "Fast mode chunk missing bm25_terms".to_string(),
                }
                .build()
            })?;
            let terms = deserialize_bm25_terms(&terms_json)?;
            IndexData::Fast { bm25_terms: terms }
        }
        "best" => {
            let blob = embedding_blob.ok_or_else(|| {
                InvalidDataSnafu {
                    message: "Best mode chunk missing embedding".to_string(),
                }
                .build()
            })?;
            let embedding = deserialize_embedding(&blob)?;
            IndexData::Best { embedding }
        }
        _ => {
            return Err(InvalidDataSnafu {
                message: format!("unknown index mode: {}", index_mode),
            }
            .build());
        }
    };

    let chunk = Chunk::builder()
        .id(ChunkId::new(uuid))
        .source(
            ChunkSource::builder()
                .file_path(
                    ForestRelativePath::try_new(file_path)
                        .map_err(|e| {
                            Box::new(e) as Box<dyn std::error::Error + Send + Sync + 'static>
                        })
                        .context(InvalidFieldSnafu {
                            field: "file_path".to_string(),
                        })?,
                )
                .repo_name(
                    RepoName::try_new(repo_name)
                        .map_err(|e| {
                            Box::new(e) as Box<dyn std::error::Error + Send + Sync + 'static>
                        })
                        .context(InvalidFieldSnafu {
                            field: "repo_name".to_string(),
                        })?,
                )
                .line_range(
                    LineRange::builder()
                        .start(
                            LineNumber::try_new(line_start as usize)
                                .map_err(|e| {
                                    Box::new(e)
                                        as Box<dyn std::error::Error + Send + Sync + 'static>
                                })
                                .context(InvalidFieldSnafu {
                                    field: "line_start".to_string(),
                                })?,
                        )
                        .line_count(
                            LineCount::try_new(line_count as usize)
                                .map_err(|e| {
                                    Box::new(e)
                                        as Box<dyn std::error::Error + Send + Sync + 'static>
                                })
                                .context(InvalidFieldSnafu {
                                    field: "line_count".to_string(),
                                })?,
                        )
                        .build(),
                )
                .build(),
        )
        .content(
            ChunkContent::builder()
                .text(content)
                .token_count(
                    TokenCount::try_new(token_count as usize)
                        .map_err(|e| {
                            Box::new(e) as Box<dyn std::error::Error + Send + Sync + 'static>
                        })
                        .context(InvalidFieldSnafu {
                            field: "token_count".to_string(),
                        })?,
                )
                .build(),
        )
        .context(context)
        .build();

    Ok(IndexedChunk::builder()
        .chunk(chunk)
        .index_data(index_data)
        .indexed_at(Timestamp::from_secs(last_modified))
        .build())
}

/// Convert f32 embedding vector to bytes for sqlite-vec storage
pub fn serialize_embedding(embedding: &[f32]) -> Vec<u8> {
    embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Convert bytes from sqlite-vec storage back to f32 embedding vector
pub fn deserialize_embedding(bytes: &[u8]) -> Result<Embedding, StorageError> {
    use crate::knowledge::constants::EMBEDDING_DIM;

    if !bytes.len().is_multiple_of(4) {
        return Err(InvalidDataSnafu {
            message: format!("Invalid embedding blob length: {}", bytes.len()),
        }
        .build());
    }

    let vec: Vec<f32> = bytes
        .chunks_exact(4)
        .map(|chunk| {
            let arr: [u8; 4] = chunk.try_into().expect("chunks_exact guarantees 4 bytes");
            f32::from_le_bytes(arr)
        })
        .collect();

    if vec.len() != EMBEDDING_DIM {
        return Err(InvalidDataSnafu {
            message: format!(
                "Invalid embedding dimension: expected {}, got {}",
                EMBEDDING_DIM,
                vec.len()
            ),
        }
        .build());
    }

    Embedding::try_new(vec)
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync + 'static>)
        .context(InvalidFieldSnafu {
            field: "embedding".to_string(),
        })
}

/// Serialize BM25 terms to JSON string
pub fn serialize_bm25_terms(
    terms: &std::collections::BTreeMap<String, u32>,
) -> Result<String, StorageError> {
    serde_json::to_string(terms).context(SerializationSnafu)
}

/// Deserialize BM25 terms from JSON string
pub fn deserialize_bm25_terms(
    json: &str,
) -> Result<std::collections::BTreeMap<String, u32>, StorageError> {
    serde_json::from_str(json).context(SerializationSnafu)
}

/// Convert ChunkContext enum to database-storable format
pub fn serialize_context(context: &ChunkContext) -> Result<(String, String), StorageError> {
    let (context_type, context_data) = match context {
        ChunkContext::Markdown(ctx) => (
            "markdown",
            serde_json::to_string(ctx).map_err(|e| {
                InvalidDataSnafu {
                    message: e.to_string(),
                }
                .build()
            })?,
        ),
        ChunkContext::RustDoc(ctx) => (
            "rust_doc",
            serde_json::to_string(ctx).map_err(|e| {
                InvalidDataSnafu {
                    message: e.to_string(),
                }
                .build()
            })?,
        ),
    };

    Ok((context_type.to_string(), context_data))
}

/// Reconstruct ChunkContext from database fields
pub fn deserialize_context(
    context_type: &str,
    context_data: &str,
) -> Result<ChunkContext, StorageError> {
    match context_type {
        "markdown" => {
            let ctx: MarkdownContext =
                serde_json::from_str(context_data).context(SerializationSnafu)?;
            Ok(ChunkContext::Markdown(ctx))
        }
        "rust_doc" => {
            let ctx: RustDocContext =
                serde_json::from_str(context_data).context(SerializationSnafu)?;
            Ok(ChunkContext::RustDoc(ctx))
        }
        _ => Err(InvalidDataSnafu {
            message: format!("unknown context type: {}", context_type),
        }
        .build()),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::knowledge::constants::EMBEDDING_DIM;
    use test_case::test_case;

    #[test]
    fn test_embedding_round_trip() {
        // Given A valid embedding with correct dimension
        let original = (0..EMBEDDING_DIM)
            .map(|i| i as f32 / 100.0)
            .collect::<Vec<_>>();
        let embedding = Embedding::try_new(original.clone()).unwrap();

        // When Serializing and deserializing
        let bytes = serialize_embedding(embedding.as_ref());
        let result = deserialize_embedding(&bytes).unwrap();

        // Then The embedding should be preserved
        assert_eq!(result.as_ref(), &original);
    }

    #[test_case(383 ; "one less than expected")]
    #[test_case(385 ; "one more than expected")]
    #[test_case(100 ; "much smaller")]
    #[test_case(1000 ; "much larger")]
    fn test_deserialize_embedding_rejects_wrong_dimension(dim: usize) {
        // Given An embedding with wrong dimension
        let wrong_size: Vec<f32> = (0..dim).map(|i| i as f32).collect();
        let bytes = serialize_embedding(&wrong_size);

        // When Deserializing
        let result = deserialize_embedding(&bytes);

        // Then It should fail with dimension error
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Invalid embedding dimension"));
    }

    #[test_case(3 ; "not multiple of 4")]
    #[test_case(7 ; "odd number")]
    fn test_deserialize_embedding_rejects_invalid_byte_length(byte_len: usize) {
        // Given A byte array with invalid length
        let bytes = vec![0u8; byte_len];

        // When Deserializing
        let result = deserialize_embedding(&bytes);

        // Then It should fail with length error
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Invalid embedding blob length"));
    }

    #[test]
    fn test_bm25_terms_round_trip() {
        // Given BM25 terms
        let mut terms = std::collections::BTreeMap::new();
        terms.insert("systemd".to_string(), 5);
        terms.insert("boot".to_string(), 3);

        // When Serializing and deserializing
        let json = serialize_bm25_terms(&terms).unwrap();
        let result = deserialize_bm25_terms(&json).unwrap();

        // Then The terms should be preserved
        assert_eq!(result, terms);
    }
}
