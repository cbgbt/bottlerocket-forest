//! Serialization between domain types and database formats
//!
//! Converts domain types to database-storable representations and reconstructs
//! domain types from database rows.

use snafu::ResultExt;

use crate::knowledge::constants::EMBEDDING_DIM;
use crate::knowledge::domain::{
    Chunk, ChunkContent, ChunkContext, ChunkHash, ChunkId, ChunkSource, Embedding, FileHash,
    ForestRelativePath, IndexedChunk, MarkdownContext, RepoName, RustDocContext, Timestamp,
    TokenCount,
};
use crate::knowledge::storage::repository::{StorageError, storage_error::*};

/// Reconstructs an IndexedChunk from a database row
///
/// Expects columns: chunk_hash (for id), chunk_hash, file_hash, file_path, repo_name,
/// context_type, context_data, content, token_count, last_modified.
///
/// Note: The first column is used to generate a deterministic UUID from the chunk_hash.
/// The file_path column may be empty string since the new schema doesn't store file_path
/// in the chunks table.
pub fn indexed_chunk_from_row(row: &rusqlite::Row) -> Result<IndexedChunk, StorageError> {
    // Column 0: chunk_hash bytes (used to generate deterministic UUID)
    let chunk_hash_for_id: Vec<u8> = row.get(0).context(DatabaseSnafu)?;
    let chunk_hash_bytes: Vec<u8> = row.get(1).context(DatabaseSnafu)?;
    let file_hash_bytes: Vec<u8> = row.get(2).context(DatabaseSnafu)?;
    let file_path: String = row.get(3).context(DatabaseSnafu)?;
    let repo_name: String = row.get(4).context(DatabaseSnafu)?;
    let context_type: String = row.get(5).context(DatabaseSnafu)?;
    let context_data: String = row.get(6).context(DatabaseSnafu)?;
    let content: String = row.get(7).context(DatabaseSnafu)?;
    let token_count: i64 = row.get(8).context(DatabaseSnafu)?;
    let last_modified: i64 = row.get(9).context(DatabaseSnafu)?;

    // Generate a deterministic UUID from the chunk_hash bytes
    let uuid = if chunk_hash_for_id.len() >= 16 {
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&chunk_hash_for_id[..16]);
        uuid::Uuid::from_bytes(bytes)
    } else {
        uuid::Uuid::nil()
    };

    let chunk_hash_array: [u8; 32] = chunk_hash_bytes.try_into().map_err(|_| {
        InvalidDataSnafu {
            message: "chunk_hash must be 32 bytes".to_string(),
        }
        .build()
    })?;

    let file_hash_array: [u8; 32] = file_hash_bytes.try_into().map_err(|_| {
        InvalidDataSnafu {
            message: "file_hash must be 32 bytes".to_string(),
        }
        .build()
    })?;

    let context = deserialize_context(&context_type, &context_data)?;

    // NOTE: Embeddings are not fetched during retrieval because they're only needed
    // during indexing. Search results extract the Chunk and discard the embedding.
    // This dummy embedding satisfies the IndexedChunk type requirement.
    let embedding = Embedding::try_new(vec![0.1; EMBEDDING_DIM])
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync + 'static>)
        .context(InvalidFieldSnafu {
            field: "embedding".to_string(),
        })?;

    // NOTE: In the content-addressed schema, file_path is not stored in the chunks table.
    // The file_path is tracked in indexed_files table, keyed by (context_id, file_path).
    // This placeholder satisfies the ChunkSource type requirement. Callers should NOT
    // rely on file_path from deserialized chunks; use indexed_files for file lookups.
    let file_path_value = if file_path.is_empty() {
        "unknown".to_string()
    } else {
        file_path
    };

    let chunk = Chunk::builder()
        .id(ChunkId::new(uuid))
        .chunk_hash(ChunkHash::new(chunk_hash_array))
        .file_hash(FileHash::new(file_hash_array))
        .source(
            ChunkSource::builder()
                .file_path(
                    ForestRelativePath::try_new(file_path_value)
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
        .embedding(embedding)
        .indexed_at(Timestamp::from_secs(last_modified))
        .build())
}

/// Converts f32 embedding vector to little-endian byte representation
pub fn serialize_embedding(embedding: &[f32]) -> Vec<u8> {
    embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Converts ChunkContext to database-storable type and JSON representation
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

/// Reconstructs ChunkContext from database type and JSON fields
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

// Tests for these serialization functions are in the parent module's integration tests,
// which verify correct behavior through full database round-trips (see mod.rs)
