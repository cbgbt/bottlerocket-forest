//! CRUD operations for chunk storage

use rusqlite::{params, Connection, OptionalExtension};
use snafu::ResultExt;

use super::serialization::{chunk_from_row, serialize_context, serialize_embedding, system_time_to_unix, unix_to_system_time};
use crate::knowledge::domain::{Chunk, ChunkId, ForestRelativePath, IndexMode};
use crate::knowledge::storage::repository::{storage_error::*, IndexMetadata, StorageError};

/// Save a single chunk to the database
pub fn save(conn: &mut Connection, chunk: &Chunk) -> Result<(), StorageError> {
    let (context_type, context_data) = serialize_context(&chunk.context)?;

    conn.execute(
        "INSERT OR REPLACE INTO chunks 
        (id, file_path, repo_name, line_start, line_end, 
         context_type, context_data, content, last_modified, index_mode)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            chunk.id.to_string(),
            chunk.source.file_path.to_string(),
            chunk.source.repo_name.to_string(),
            chunk.source.line_range.start.into_inner() as i64,
            chunk.source.line_range.line_count.into_inner() as i64,
            context_type,
            context_data,
            chunk.content.text,
            system_time_to_unix(chunk.indexed_at),
            "fast",
        ],
    )
    .context(DatabaseSnafu)?;

    if let Some(embedding) = &chunk.embedding {
        let embedding_blob = serialize_embedding(embedding);
        conn.execute(
            "INSERT OR REPLACE INTO vec_chunks (chunk_id, embedding) VALUES (?1, ?2)",
            params![chunk.id.to_string(), embedding_blob],
        )
        .context(DatabaseSnafu)?;
    }

    Ok(())
}

/// Save multiple chunks in a transaction
pub fn save_batch(conn: &mut Connection, chunks: &[Chunk]) -> Result<(), StorageError> {
    let tx = conn.transaction().context(DatabaseSnafu)?;

    for chunk in chunks {
        let (context_type, context_data) = serialize_context(&chunk.context)?;

        tx.execute(
            "INSERT OR REPLACE INTO chunks 
            (id, file_path, repo_name, line_start, line_end, 
             context_type, context_data, content, last_modified, index_mode)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                chunk.id.to_string(),
                chunk.source.file_path.to_string(),
                chunk.source.repo_name.to_string(),
                chunk.source.line_range.start.into_inner() as i64,
                chunk.source.line_range.line_count.into_inner() as i64,
                context_type,
                context_data,
                chunk.content.text,
                system_time_to_unix(chunk.indexed_at),
                "fast",
            ],
        )
        .context(DatabaseSnafu)?;

        if let Some(embedding) = &chunk.embedding {
            let embedding_blob = serialize_embedding(embedding);
            tx.execute(
                "INSERT OR REPLACE INTO vec_chunks (chunk_id, embedding) VALUES (?1, ?2)",
                params![chunk.id.to_string(), embedding_blob],
            )
            .context(DatabaseSnafu)?;
        }
    }

    tx.commit().context(DatabaseSnafu)?;

    Ok(())
}

/// Find a chunk by its ID
pub fn find_by_id(conn: &Connection, id: &ChunkId) -> Result<Option<Chunk>, StorageError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, file_path, repo_name, line_start, line_end, 
                    context_type, context_data, content, last_modified
             FROM chunks WHERE id = ?1",
        )
        .context(DatabaseSnafu)?;

    stmt.query_row(params![id.to_string()], |row| chunk_from_row!(row))
        .optional()
        .context(DatabaseSnafu)
}

/// Find all chunks from a specific file
pub fn find_by_file(conn: &Connection, path: &ForestRelativePath) -> Result<Vec<Chunk>, StorageError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, file_path, repo_name, line_start, line_end, 
                    context_type, context_data, content, last_modified
             FROM chunks WHERE file_path = ?1",
        )
        .context(DatabaseSnafu)?;

    stmt.query_map(params![path.to_string()], |row| chunk_from_row!(row))
        .context(DatabaseSnafu)?
        .collect::<Result<Vec<_>, _>>()
        .context(DatabaseSnafu)
}

/// Find all chunks in the database
pub fn find_all(conn: &Connection) -> Result<Vec<Chunk>, StorageError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, file_path, repo_name, line_start, line_end, 
                    context_type, context_data, content, last_modified
             FROM chunks",
        )
        .context(DatabaseSnafu)?;

    stmt.query_map([], |row| chunk_from_row!(row))
        .context(DatabaseSnafu)?
        .collect::<Result<Vec<_>, _>>()
        .context(DatabaseSnafu)
}

/// Delete all chunks from a specific file
pub fn delete_by_file(conn: &mut Connection, path: &ForestRelativePath) -> Result<usize, StorageError> {
    let count = conn
        .execute(
            "DELETE FROM chunks WHERE file_path = ?1",
            params![path.to_string()],
        )
        .context(DatabaseSnafu)?;

    Ok(count)
}

/// Delete all chunks from the database
pub fn clear(conn: &mut Connection) -> Result<usize, StorageError> {
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

    Ok(IndexMetadata::builder()
        .mode(mode)
        .last_build(unix_to_system_time(last_build_unix))
        .chunk_count(chunk_count)
        .file_count(file_count)
        .build())
}

/// Set index metadata
pub fn set_metadata(conn: &mut Connection, metadata: &IndexMetadata) -> Result<(), StorageError> {
    conn.execute(
        "INSERT OR REPLACE INTO index_metadata (key, value) VALUES ('mode', ?1)",
        params![metadata.mode.to_string()],
    )
    .context(DatabaseSnafu)?;

    conn.execute(
        "INSERT OR REPLACE INTO index_metadata (key, value) VALUES ('last_build', ?1)",
        params![system_time_to_unix(metadata.last_build).to_string()],
    )
    .context(DatabaseSnafu)?;

    Ok(())
}
