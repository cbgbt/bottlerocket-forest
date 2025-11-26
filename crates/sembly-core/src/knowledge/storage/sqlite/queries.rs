//! CRUD operations for chunk storage
//!
//! Implements database queries for persisting and retrieving indexed chunks.

use rusqlite::{Connection, OptionalExtension};
use snafu::ResultExt;

use super::serialization::{indexed_chunk_from_row, serialize_context, serialize_embedding};
use crate::knowledge::domain::{
    ChunkHash, ChunkId, EmbeddingModelConfig, FileHash, ForestRelativePath, IndexMetadata,
    IndexedChunk, Timestamp,
};
use crate::knowledge::storage::repository::{StorageError, storage_error::*};

/// Persists a single indexed chunk to the database
pub fn save(conn: &mut Connection, indexed_chunk: &IndexedChunk) -> Result<(), StorageError> {
    let chunk = &indexed_chunk.chunk;
    let (context_type, context_data) = serialize_context(&chunk.context)?;
    let embedding_blob = serialize_embedding(&indexed_chunk.embedding);

    conn.execute(
        "INSERT OR REPLACE INTO chunks 
        (chunk_hash, file_hash, repo_name, 
         context_type, context_data, content, token_count, last_modified)
        VALUES (:chunk_hash, :file_hash, :repo_name, 
                :context_type, :context_data, :content, :token_count, :last_modified)",
        rusqlite::named_params! {
            ":chunk_hash": chunk.chunk_hash.as_bytes().as_slice(),
            ":file_hash": chunk.file_hash.as_bytes().as_slice(),
            ":repo_name": chunk.source.repo_name.to_string(),
            ":context_type": context_type,
            ":context_data": context_data,
            ":content": chunk.content.text,
            ":token_count": chunk.content.token_count.into_inner() as i64,
            ":last_modified": indexed_chunk.indexed_at.as_secs(),
        },
    )
    .context(DatabaseSnafu)?;

    conn.execute(
        "INSERT OR REPLACE INTO vec_chunks (chunk_hash, embedding) VALUES (:chunk_hash, :embedding)",
        rusqlite::named_params! {
            ":chunk_hash": chunk.chunk_hash.to_string(),
            ":embedding": embedding_blob,
        },
    )
    .context(DatabaseSnafu)?;

    Ok(())
}

/// Persists multiple indexed chunks in a single transaction
pub fn save_batch(conn: &mut Connection, chunks: &[IndexedChunk]) -> Result<(), StorageError> {
    let tx = conn.transaction().context(DatabaseSnafu)?;

    for indexed_chunk in chunks {
        let chunk = &indexed_chunk.chunk;
        let (context_type, context_data) = serialize_context(&chunk.context)?;
        let embedding_blob = serialize_embedding(&indexed_chunk.embedding);

        tx.execute(
            "INSERT OR REPLACE INTO chunks 
            (chunk_hash, file_hash, repo_name, 
             context_type, context_data, content, token_count, last_modified)
            VALUES (:chunk_hash, :file_hash, :repo_name, 
                    :context_type, :context_data, :content, :token_count, :last_modified)",
            rusqlite::named_params! {
                ":chunk_hash": chunk.chunk_hash.as_bytes().as_slice(),
                ":file_hash": chunk.file_hash.as_bytes().as_slice(),
                ":repo_name": chunk.source.repo_name.to_string(),
                ":context_type": context_type,
                ":context_data": context_data,
                ":content": chunk.content.text,
                ":token_count": chunk.content.token_count.into_inner() as i64,
                ":last_modified": indexed_chunk.indexed_at.as_secs(),
            },
        )
        .context(DatabaseSnafu)?;

        tx.execute(
            "INSERT OR REPLACE INTO vec_chunks (chunk_hash, embedding) VALUES (:chunk_hash, :embedding)",
            rusqlite::named_params! {
                ":chunk_hash": chunk.chunk_hash.to_string(),
                ":embedding": embedding_blob,
            },
        )
        .context(DatabaseSnafu)?;
    }

    tx.commit().context(DatabaseSnafu)?;

    Ok(())
}

/// Retrieves an indexed chunk by its ChunkId
///
/// Note: In the new content-addressed schema, chunks are keyed by chunk_hash.
/// This function searches all chunks and matches by the deterministic UUID
/// derived from chunk_hash. For direct chunk_hash lookups, use find_by_chunk_hash.
#[allow(dead_code)] // Deprecated: use find_by_chunk_hash in new schema
pub fn find_by_id(conn: &Connection, id: &ChunkId) -> Result<Option<IndexedChunk>, StorageError> {
    // In the new schema, we don't have a direct id column.
    // We need to find by iterating and matching the derived UUID.
    // This is inefficient but maintains backward compatibility.
    let all = find_all(conn)?;
    Ok(all.into_iter().find(|c| c.chunk.id == *id))
}

/// Retrieves an indexed chunk by its chunk hash
#[allow(dead_code)] // Used by indexer in Commit 10
pub fn find_by_chunk_hash(
    conn: &Connection,
    chunk_hash: &ChunkHash,
) -> Result<Option<IndexedChunk>, StorageError> {
    let mut stmt = conn
        .prepare(
            "SELECT c.chunk_hash, c.chunk_hash, c.file_hash, 
                    '' as file_path, c.repo_name, 
                    c.context_type, c.context_data, c.content, c.token_count, c.last_modified
             FROM chunks c
             WHERE c.chunk_hash = :chunk_hash",
        )
        .context(DatabaseSnafu)?;

    stmt.query_row(
        rusqlite::named_params! { ":chunk_hash": chunk_hash.as_bytes().as_slice() },
        |row| {
            indexed_chunk_from_row(row)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
        },
    )
    .optional()
    .context(DatabaseSnafu)
}

/// Retrieves all indexed chunks from a specific file by file path
///
/// Note: In the new content-addressed schema, chunks are keyed by file_hash, not file_path.
/// This function returns an empty vec since file_path is no longer stored in chunks table.
/// For file-based lookups, use find_by_file_hash instead.
pub fn find_by_file(
    _conn: &Connection,
    _path: &ForestRelativePath,
) -> Result<Vec<IndexedChunk>, StorageError> {
    // In the new schema, file_path is not stored in chunks table.
    // Return empty vec for backward compatibility.
    Ok(vec![])
}

/// Retrieves all indexed chunks from a specific file by file_hash
#[allow(dead_code)] // Used by indexer in Commit 10
pub fn find_by_file_hash(
    conn: &Connection,
    file_hash: &FileHash,
) -> Result<Vec<IndexedChunk>, StorageError> {
    let mut stmt = conn
        .prepare(
            "SELECT c.chunk_hash, c.chunk_hash, c.file_hash, 
                    '' as file_path, c.repo_name, 
                    c.context_type, c.context_data, c.content, c.token_count, c.last_modified
             FROM chunks c
             WHERE c.file_hash = :file_hash",
        )
        .context(DatabaseSnafu)?;

    stmt.query_map(
        rusqlite::named_params! { ":file_hash": file_hash.as_bytes().as_slice() },
        |row| {
            indexed_chunk_from_row(row)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
        },
    )
    .context(DatabaseSnafu)?
    .collect::<Result<Vec<_>, _>>()
    .context(DatabaseSnafu)
}

/// Retrieves all indexed chunks from the database
pub fn find_all(conn: &Connection) -> Result<Vec<IndexedChunk>, StorageError> {
    let mut stmt = conn
        .prepare(
            "SELECT c.chunk_hash, c.chunk_hash, c.file_hash, 
                    '' as file_path, c.repo_name, 
                    c.context_type, c.context_data, c.content, c.token_count, c.last_modified
             FROM chunks c",
        )
        .context(DatabaseSnafu)?;

    stmt.query_map([], |row| {
        indexed_chunk_from_row(row)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
    })
    .context(DatabaseSnafu)?
    .collect::<Result<Vec<_>, _>>()
    .context(DatabaseSnafu)
}

/// Retrieves file hashes and their most recent indexing timestamps
///
/// Note: This function returns an empty map because the new schema stores
/// file_hash in chunks, not file_path. The indexed_files table is now
/// context-scoped and should be queried through the context repository.
pub fn get_indexed_files(
    _conn: &Connection,
) -> Result<std::collections::HashMap<ForestRelativePath, Timestamp>, StorageError> {
    Ok(std::collections::HashMap::new())
}

/// Removes all chunks associated with a specific file path
///
/// Note: In the new schema, chunks are keyed by file_hash, not file_path.
/// This function returns 0 for backward compatibility.
/// For file-based deletion, use delete_by_file_hash instead.
pub fn delete_by_file(
    _conn: &mut Connection,
    _path: &ForestRelativePath,
) -> Result<usize, StorageError> {
    // In the new schema, file_path is not stored in chunks table.
    Ok(0)
}

/// Removes all chunks associated with a specific file_hash
#[allow(dead_code)] // Used by indexer in Commit 10
pub fn delete_by_file_hash(
    conn: &mut Connection,
    file_hash: &FileHash,
) -> Result<usize, StorageError> {
    // First get the chunk_hashes to delete from vec_chunks
    let chunk_hashes: Vec<Vec<u8>> = {
        let mut stmt = conn
            .prepare("SELECT chunk_hash FROM chunks WHERE file_hash = :file_hash")
            .context(DatabaseSnafu)?;
        stmt.query_map(
            rusqlite::named_params! { ":file_hash": file_hash.as_bytes().as_slice() },
            |row| row.get(0),
        )
        .context(DatabaseSnafu)?
        .collect::<Result<Vec<_>, _>>()
        .context(DatabaseSnafu)?
    };

    // Delete from vec_chunks using chunk_hash string representation
    for chunk_hash_bytes in &chunk_hashes {
        let chunk_hash_hex = chunk_hash_bytes
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>();
        conn.execute(
            "DELETE FROM vec_chunks WHERE chunk_hash = :chunk_hash",
            rusqlite::named_params! { ":chunk_hash": chunk_hash_hex },
        )
        .context(DatabaseSnafu)?;
    }

    let count = conn
        .execute(
            "DELETE FROM chunks WHERE file_hash = :file_hash",
            rusqlite::named_params! { ":file_hash": file_hash.as_bytes().as_slice() },
        )
        .context(DatabaseSnafu)?;

    Ok(count)
}

/// Removes all chunks from the database
pub fn clear(conn: &mut Connection) -> Result<usize, StorageError> {
    conn.execute("DELETE FROM vec_chunks", [])
        .context(DatabaseSnafu)?;

    let count = conn
        .execute("DELETE FROM chunks", [])
        .context(DatabaseSnafu)?;

    Ok(count)
}

/// Retrieves index metadata including build time and model configuration
pub fn get_metadata(conn: &Connection) -> Result<IndexMetadata, StorageError> {
    let last_build_unix: i64 = conn
        .query_row(
            "SELECT value FROM index_metadata WHERE key = 'last_build'",
            [],
            |row| row.get(0),
        )
        .optional()
        .context(DatabaseSnafu)?
        .and_then(|s: String| s.parse().ok())
        .unwrap_or(0);

    let chunk_count: usize = conn
        .query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))
        .context(DatabaseSnafu)?;

    let file_count: usize = conn
        .query_row("SELECT COUNT(DISTINCT file_hash) FROM chunks", [], |row| {
            row.get(0)
        })
        .context(DatabaseSnafu)?;

    let model_name: String = conn
        .query_row(
            "SELECT value FROM index_metadata WHERE key = 'model_name'",
            [],
            |row| row.get(0),
        )
        .optional()
        .context(DatabaseSnafu)?
        .unwrap_or_else(|| crate::knowledge::constants::DEFAULT_TOKENIZER_MODEL.to_string());

    let embedding_dim: usize = conn
        .query_row(
            "SELECT value FROM index_metadata WHERE key = 'embedding_dim'",
            [],
            |row| row.get(0),
        )
        .optional()
        .context(DatabaseSnafu)?
        .and_then(|s: String| s.parse().ok())
        .unwrap_or(crate::knowledge::constants::EMBEDDING_DIM);

    let max_tokens: usize = conn
        .query_row(
            "SELECT value FROM index_metadata WHERE key = 'max_tokens'",
            [],
            |row| row.get(0),
        )
        .optional()
        .context(DatabaseSnafu)?
        .and_then(|s: String| s.parse().ok())
        .unwrap_or(crate::knowledge::constants::DEFAULT_MAX_CHUNK_TOKENS);

    let overlap_tokens: usize = conn
        .query_row(
            "SELECT value FROM index_metadata WHERE key = 'overlap_tokens'",
            [],
            |row| row.get(0),
        )
        .optional()
        .context(DatabaseSnafu)?
        .and_then(|s: String| s.parse().ok())
        .unwrap_or(crate::knowledge::constants::DEFAULT_CHUNK_OVERLAP_TOKENS);

    let model_config = EmbeddingModelConfig::builder()
        .model_name(model_name)
        .embedding_dim(embedding_dim)
        .max_tokens(max_tokens)
        .overlap_tokens(overlap_tokens)
        .build();

    let last_build = std::time::UNIX_EPOCH + std::time::Duration::from_secs(last_build_unix as u64);

    Ok(IndexMetadata::builder()
        .last_build(last_build)
        .chunk_count(chunk_count)
        .file_count(file_count)
        .model_config(model_config)
        .build())
}

/// Updates index metadata in the database
pub fn set_metadata(conn: &mut Connection, metadata: &IndexMetadata) -> Result<(), StorageError> {
    let last_build_unix = metadata
        .last_build
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    conn.execute(
        "INSERT OR REPLACE INTO index_metadata (key, value) VALUES ('last_build', :last_build)",
        rusqlite::named_params! { ":last_build": last_build_unix.to_string() },
    )
    .context(DatabaseSnafu)?;

    conn.execute(
        "INSERT OR REPLACE INTO index_metadata (key, value) VALUES ('model_name', :model_name)",
        rusqlite::named_params! { ":model_name": metadata.model_config.model_name },
    )
    .context(DatabaseSnafu)?;

    conn.execute(
        "INSERT OR REPLACE INTO index_metadata (key, value) VALUES ('embedding_dim', :embedding_dim)",
        rusqlite::named_params! { ":embedding_dim": metadata.model_config.embedding_dim.to_string() },
    )
    .context(DatabaseSnafu)?;

    conn.execute(
        "INSERT OR REPLACE INTO index_metadata (key, value) VALUES ('max_tokens', :max_tokens)",
        rusqlite::named_params! { ":max_tokens": metadata.model_config.max_tokens.to_string() },
    )
    .context(DatabaseSnafu)?;

    conn.execute(
        "INSERT OR REPLACE INTO index_metadata (key, value) VALUES ('overlap_tokens', :overlap_tokens)",
        rusqlite::named_params! { ":overlap_tokens": metadata.model_config.overlap_tokens.to_string() },
    )
    .context(DatabaseSnafu)?;

    Ok(())
}
