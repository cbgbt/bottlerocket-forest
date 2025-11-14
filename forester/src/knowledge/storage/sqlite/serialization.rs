//! Serialization helpers for converting between domain types and database formats

use snafu::ResultExt;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::knowledge::domain::{ChunkContext, MarkdownContext, RustDocContext};
use crate::knowledge::storage::repository::{StorageError, storage_error::*};

/// Macro to parse a Chunk directly from a rusqlite::Row
///
/// Expects columns in order: id, file_path, repo_name, line_start, line_count,
/// context_type, context_data, content, token_count, last_modified, bm25_terms, index_mode, embedding
///
/// Returns Result<Chunk, rusqlite::Error> for use in query_map closures
macro_rules! chunk_from_row {
    ($row:expr) => {{
        (|| -> Result<crate::knowledge::domain::Chunk, rusqlite::Error> {
            use crate::knowledge::storage::repository::storage_error::*;
            use snafu::ResultExt;

            let id_str: String = $row.get(0)?;
            let file_path: String = $row.get(1)?;
            let repo_name: String = $row.get(2)?;
            let line_start: i64 = $row.get(3)?;
            let line_count: i64 = $row.get(4)?;
            let context_type: String = $row.get(5)?;
            let context_data: String = $row.get(6)?;
            let content: String = $row.get(7)?;
            let token_count: i64 = $row.get(8)?;
            let last_modified: i64 = $row.get(9)?;
            let bm25_terms: Option<String> = $row.get(10)?;
            let index_mode: String = $row.get(11)?;
            let embedding_blob: Option<Vec<u8>> = $row.get(12)?;

            let uuid = uuid::Uuid::parse_str(&id_str)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync + 'static>)
                .context(InvalidFieldSnafu {
                    field: "chunk_id".to_string(),
                })
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

            let context = crate::knowledge::storage::sqlite::serialization::deserialize_context(
                &context_type,
                &context_data,
            )
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

            let index_data = match index_mode.as_str() {
                "fast" => {
                    let terms_json = bm25_terms.ok_or_else(|| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(
                            InvalidDataSnafu {
                                message: "Fast mode chunk missing bm25_terms".to_string(),
                            }
                            .build(),
                        ))
                    })?;
                    let terms =
                        crate::knowledge::storage::sqlite::serialization::deserialize_bm25_terms(
                            &terms_json,
                        )
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                    crate::knowledge::domain::IndexData::Fast { bm25_terms: terms }
                }
                "best" => {
                    let blob = embedding_blob.ok_or_else(|| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(
                            InvalidDataSnafu {
                                message: "Best mode chunk missing embedding".to_string(),
                            }
                            .build(),
                        ))
                    })?;
                    let embedding =
                        crate::knowledge::storage::sqlite::serialization::deserialize_embedding(
                            &blob,
                        )
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                    crate::knowledge::domain::IndexData::Best { embedding }
                }
                _ => {
                    return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                        InvalidDataSnafu {
                            message: format!("unknown index mode: {}", index_mode),
                        }
                        .build(),
                    )));
                }
            };

            Ok(crate::knowledge::domain::Chunk::builder()
                .id(crate::knowledge::domain::ChunkId::new(uuid))
                .source(
                    crate::knowledge::domain::ChunkSource::builder()
                        .file_path(
                            crate::knowledge::domain::ForestRelativePath::try_new(file_path)
                                .map_err(|e| {
                                    Box::new(e)
                                        as Box<dyn std::error::Error + Send + Sync + 'static>
                                })
                                .context(InvalidFieldSnafu {
                                    field: "file_path".to_string(),
                                })
                                .map_err(|e| {
                                    rusqlite::Error::ToSqlConversionFailure(Box::new(e))
                                })?,
                        )
                        .repo_name(
                            crate::knowledge::domain::RepoName::try_new(repo_name)
                                .map_err(|e| {
                                    Box::new(e)
                                        as Box<dyn std::error::Error + Send + Sync + 'static>
                                })
                                .context(InvalidFieldSnafu {
                                    field: "repo_name".to_string(),
                                })
                                .map_err(|e| {
                                    rusqlite::Error::ToSqlConversionFailure(Box::new(e))
                                })?,
                        )
                        .line_range(
                            crate::knowledge::domain::LineRange::builder()
                                .start(
                                    crate::knowledge::domain::LineNumber::try_new(
                                        line_start as usize,
                                    )
                                    .map_err(|e| {
                                        Box::new(e)
                                            as Box<dyn std::error::Error + Send + Sync + 'static>
                                    })
                                    .context(InvalidFieldSnafu {
                                        field: "line_start".to_string(),
                                    })
                                    .map_err(|e| {
                                        rusqlite::Error::ToSqlConversionFailure(Box::new(e))
                                    })?,
                                )
                                .line_count(
                                    crate::knowledge::domain::LineCount::try_new(
                                        line_count as usize,
                                    )
                                    .map_err(|e| {
                                        Box::new(e)
                                            as Box<dyn std::error::Error + Send + Sync + 'static>
                                    })
                                    .context(InvalidFieldSnafu {
                                        field: "line_count".to_string(),
                                    })
                                    .map_err(|e| {
                                        rusqlite::Error::ToSqlConversionFailure(Box::new(e))
                                    })?,
                                )
                                .build(),
                        )
                        .build(),
                )
                .content(
                    crate::knowledge::domain::ChunkContent::builder()
                        .text(content.clone())
                        .token_count(
                            crate::knowledge::domain::TokenCount::try_new(token_count as usize)
                                .map_err(|e| {
                                    Box::new(e)
                                        as Box<dyn std::error::Error + Send + Sync + 'static>
                                })
                                .context(InvalidFieldSnafu {
                                    field: "token_count".to_string(),
                                })
                                .map_err(|e| {
                                    rusqlite::Error::ToSqlConversionFailure(Box::new(e))
                                })?,
                        )
                        .build(),
                )
                .context(context)
                .indexed_at(
                    crate::knowledge::storage::sqlite::serialization::unix_to_system_time(
                        last_modified,
                    ),
                )
                .index_data(index_data)
                .build())
        })()
    }};
}

pub(super) use chunk_from_row;

/// Convert f32 embedding vector to bytes for sqlite-vec storage
pub fn serialize_embedding(embedding: &[f32]) -> Vec<u8> {
    embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Convert bytes from sqlite-vec storage back to f32 embedding vector
pub fn deserialize_embedding(
    bytes: &[u8],
) -> Result<crate::knowledge::domain::Embedding, StorageError> {
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

    crate::knowledge::domain::Embedding::try_new(vec)
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

/// Convert SystemTime to Unix timestamp for database storage
pub fn system_time_to_unix(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Convert Unix timestamp from database to SystemTime
pub fn unix_to_system_time(unix: i64) -> SystemTime {
    UNIX_EPOCH + std::time::Duration::from_secs(unix as u64)
}
