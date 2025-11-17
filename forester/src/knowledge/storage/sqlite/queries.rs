//! CRUD operations for chunk storage

use rusqlite::{Connection, OptionalExtension};
use snafu::ResultExt;

use super::serialization::{
    indexed_chunk_from_row, serialize_bm25_terms, serialize_context, serialize_embedding,
};
use crate::knowledge::domain::{
    ChunkId, EmbeddingModelConfig, ForestRelativePath, IndexData, IndexMetadata, IndexMode,
    IndexedChunk, Timestamp,
};
use crate::knowledge::storage::repository::{StorageError, storage_error::*};

/// Save a single indexed chunk to the database
pub fn save(conn: &mut Connection, indexed_chunk: &IndexedChunk) -> Result<(), StorageError> {
    let chunk = &indexed_chunk.chunk;
    let (context_type, context_data) = serialize_context(&chunk.context)?;
    let mode = indexed_chunk.index_data.mode();

    let (bm25_terms, embedding_blob) = match &indexed_chunk.index_data {
        IndexData::Fast { bm25_terms } => {
            let terms_json = serialize_bm25_terms(bm25_terms)?;
            (Some(terms_json), None)
        }
        IndexData::Best { embedding } => {
            let blob = serialize_embedding(embedding);
            (None, Some(blob))
        }
    };

    conn.execute(
        "INSERT OR REPLACE INTO chunks 
        (id, file_path, repo_name, line_start, line_count, 
         context_type, context_data, content, token_count, last_modified, bm25_terms, index_mode)
        VALUES (:id, :file_path, :repo_name, :line_start, :line_count, 
                :context_type, :context_data, :content, :token_count, :last_modified, :bm25_terms, :index_mode)",
        rusqlite::named_params! {
            ":id": chunk.id.to_string(),
            ":file_path": chunk.source.file_path.to_string(),
            ":repo_name": chunk.source.repo_name.to_string(),
            ":line_start": chunk.source.line_range.start.into_inner() as i64,
            ":line_count": chunk.source.line_range.line_count.into_inner() as i64,
            ":context_type": context_type,
            ":context_data": context_data,
            ":content": chunk.content.text,
            ":token_count": chunk.content.token_count.into_inner() as i64,
            ":last_modified": indexed_chunk.indexed_at.as_secs(),
            ":bm25_terms": bm25_terms,
            ":index_mode": mode.to_string(),
        },
    )
    .context(DatabaseSnafu)?;

    if let Some(blob) = embedding_blob {
        conn.execute(
            "INSERT OR REPLACE INTO vec_chunks (chunk_id, embedding) VALUES (:chunk_id, :embedding)",
            rusqlite::named_params! {
                ":chunk_id": chunk.id.to_string(),
                ":embedding": blob,
            },
        )
        .context(DatabaseSnafu)?;
    }

    Ok(())
}

/// Save multiple indexed chunks in a transaction
///
/// If an error occurs during the batch operation, the transaction is automatically
/// rolled back when `tx` is dropped (Rust's RAII pattern), ensuring atomicity.
pub fn save_batch(
    conn: &mut Connection,
    indexed_chunks: &[IndexedChunk],
) -> Result<(), StorageError> {
    let tx = conn.transaction().context(DatabaseSnafu)?;

    for indexed_chunk in indexed_chunks {
        let chunk = &indexed_chunk.chunk;
        let (context_type, context_data) = serialize_context(&chunk.context)?;
        let mode = indexed_chunk.index_data.mode();

        let (bm25_terms, embedding_blob) = match &indexed_chunk.index_data {
            IndexData::Fast { bm25_terms } => {
                let terms_json = serialize_bm25_terms(bm25_terms)?;
                (Some(terms_json), None)
            }
            IndexData::Best { embedding } => {
                let blob = serialize_embedding(embedding);
                (None, Some(blob))
            }
        };

        tx.execute(
            "INSERT OR REPLACE INTO chunks 
            (id, file_path, repo_name, line_start, line_count, 
             context_type, context_data, content, token_count, last_modified, bm25_terms, index_mode)
            VALUES (:id, :file_path, :repo_name, :line_start, :line_count, 
                    :context_type, :context_data, :content, :token_count, :last_modified, :bm25_terms, :index_mode)",
            rusqlite::named_params! {
                ":id": chunk.id.to_string(),
                ":file_path": chunk.source.file_path.to_string(),
                ":repo_name": chunk.source.repo_name.to_string(),
                ":line_start": chunk.source.line_range.start.into_inner() as i64,
                ":line_count": chunk.source.line_range.line_count.into_inner() as i64,
                ":context_type": context_type,
                ":context_data": context_data,
                ":content": chunk.content.text,
                ":token_count": chunk.content.token_count.into_inner() as i64,
                ":last_modified": indexed_chunk.indexed_at.as_secs(),
                ":bm25_terms": bm25_terms,
                ":index_mode": mode.to_string(),
            },
        )
        .context(DatabaseSnafu)?;

        if let Some(blob) = embedding_blob {
            tx.execute(
                "INSERT OR REPLACE INTO vec_chunks (chunk_id, embedding) VALUES (:chunk_id, :embedding)",
                rusqlite::named_params! {
                    ":chunk_id": chunk.id.to_string(),
                    ":embedding": blob,
                },
            )
            .context(DatabaseSnafu)?;
        }
    }

    tx.commit().context(DatabaseSnafu)?;

    Ok(())
}

/// Find an indexed chunk by its ID
pub fn find_by_id(conn: &Connection, id: &ChunkId) -> Result<Option<IndexedChunk>, StorageError> {
    let mut stmt = conn
        .prepare(
            "SELECT c.id, c.file_path, c.repo_name, c.line_start, c.line_count, 
                    c.context_type, c.context_data, c.content, c.token_count, c.last_modified, 
                    c.bm25_terms, c.index_mode, v.embedding
             FROM chunks c
             LEFT JOIN vec_chunks v ON c.id = v.chunk_id
             WHERE c.id = :id",
        )
        .context(DatabaseSnafu)?;

    stmt.query_row(rusqlite::named_params! { ":id": id.to_string() }, |row| {
        indexed_chunk_from_row(row)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
    })
    .optional()
    .context(DatabaseSnafu)
}

/// Find all indexed chunks from a specific file
pub fn find_by_file(
    conn: &Connection,
    path: &ForestRelativePath,
) -> Result<Vec<IndexedChunk>, StorageError> {
    let mut stmt = conn
        .prepare(
            "SELECT c.id, c.file_path, c.repo_name, c.line_start, c.line_count, 
                    c.context_type, c.context_data, c.content, c.token_count, c.last_modified, 
                    c.bm25_terms, c.index_mode, v.embedding
             FROM chunks c
             LEFT JOIN vec_chunks v ON c.id = v.chunk_id
             WHERE c.file_path = :file_path",
        )
        .context(DatabaseSnafu)?;

    stmt.query_map(
        rusqlite::named_params! { ":file_path": path.to_string() },
        |row| {
            indexed_chunk_from_row(row)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
        },
    )
    .context(DatabaseSnafu)?
    .collect::<Result<Vec<_>, _>>()
    .context(DatabaseSnafu)
}

/// Find all indexed chunks in the database
pub fn find_all(conn: &Connection) -> Result<Vec<IndexedChunk>, StorageError> {
    let mut stmt = conn
        .prepare(
            "SELECT c.id, c.file_path, c.repo_name, c.line_start, c.line_count, 
                    c.context_type, c.context_data, c.content, c.token_count, c.last_modified, 
                    c.bm25_terms, c.index_mode, v.embedding
             FROM chunks c
             LEFT JOIN vec_chunks v ON c.id = v.chunk_id",
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

/// Get indexed file metadata (path and last indexed timestamp)
pub fn get_indexed_files(
    conn: &Connection,
) -> Result<std::collections::HashMap<ForestRelativePath, Timestamp>, StorageError> {
    let mut stmt = conn
        .prepare("SELECT file_path, MAX(last_modified) FROM chunks GROUP BY file_path")
        .context(DatabaseSnafu)?;

    let rows = stmt
        .query_map([], |row| {
            let path_str: String = row.get(0)?;
            let timestamp: i64 = row.get(1)?;
            Ok((path_str, timestamp))
        })
        .context(DatabaseSnafu)?;

    let mut result = std::collections::HashMap::new();
    for row in rows {
        let (path_str, timestamp) = row.context(DatabaseSnafu)?;
        let path =
            ForestRelativePath::try_new(path_str).map_err(|_| StorageError::InvalidData {
                message: "invalid file path in database".to_string(),
            })?;
        result.insert(path, Timestamp::from_secs(timestamp));
    }

    Ok(result)
}

/// Delete all chunks from a specific file
pub fn delete_by_file(
    conn: &mut Connection,
    path: &ForestRelativePath,
) -> Result<usize, StorageError> {
    conn.execute(
        "DELETE FROM vec_chunks WHERE chunk_id IN (SELECT id FROM chunks WHERE file_path = :file_path)",
        rusqlite::named_params! { ":file_path": path.to_string() },
    )
    .context(DatabaseSnafu)?;

    let count = conn
        .execute(
            "DELETE FROM chunks WHERE file_path = :file_path",
            rusqlite::named_params! { ":file_path": path.to_string() },
        )
        .context(DatabaseSnafu)?;

    Ok(count)
}

/// Delete all chunks from the database
pub fn clear(conn: &mut Connection) -> Result<usize, StorageError> {
    conn.execute("DELETE FROM vec_chunks", [])
        .context(DatabaseSnafu)?;

    let count = conn
        .execute("DELETE FROM chunks", [])
        .context(DatabaseSnafu)?;

    Ok(count)
}

/// Get index metadata
pub fn get_metadata(conn: &Connection) -> Result<IndexMetadata, StorageError> {
    let mode_str: String = conn
        .query_row(
            "SELECT value FROM index_metadata WHERE key = 'mode'",
            [],
            |row| row.get(0),
        )
        .optional()
        .context(DatabaseSnafu)?
        .unwrap_or_else(|| "fast".to_string());

    let mode = mode_str.parse::<IndexMode>().map_err(|e| {
        InvalidDataSnafu {
            message: e.to_string(),
        }
        .build()
    })?;

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
        .query_row("SELECT COUNT(DISTINCT file_path) FROM chunks", [], |row| {
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
        .mode(mode)
        .last_build(last_build)
        .chunk_count(chunk_count)
        .file_count(file_count)
        .model_config(model_config)
        .build())
}

/// Set index metadata
pub fn set_metadata(conn: &mut Connection, metadata: &IndexMetadata) -> Result<(), StorageError> {
    conn.execute(
        "INSERT OR REPLACE INTO index_metadata (key, value) VALUES ('mode', :mode)",
        rusqlite::named_params! { ":mode": metadata.mode.to_string() },
    )
    .context(DatabaseSnafu)?;

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
