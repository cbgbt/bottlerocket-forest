//! Semantic search implementation using vector embeddings
//!
//! Provides k-nearest-neighbor search using sqlite-vec with cosine distance.
//!
//! # Implementation Notes
//!
//! Similarity scores are computed as `1.0 - distance` where distance is cosine distance.
//! This assumes normalized embeddings (as produced by fastembed). Non-normalized embeddings
//! may produce scores outside [0, 1].

use rusqlite::Connection;
use snafu::ResultExt;

use super::serialization::{indexed_chunk_from_row, serialize_embedding};
use crate::knowledge::domain::IndexedChunk;
use crate::knowledge::storage::repository::{StorageError, storage_error::*};

/// Performs k-nearest-neighbor search using cosine distance
///
/// Returns chunks ranked by similarity score in descending order. Similarity scores
/// are computed as `1.0 - distance` where distance is the cosine distance between
/// normalized embeddings.
pub fn search_semantic(
    conn: &Connection,
    query_embedding: &[f32],
    limit: usize,
) -> Result<Vec<(IndexedChunk, f32)>, StorageError> {
    let embedding_blob = serialize_embedding(query_embedding);

    let mut stmt = conn
        .prepare(
            "SELECT c.id, c.file_path, c.repo_name,
                    c.context_type, c.context_data, c.content, c.token_count, c.last_modified,
                    v.distance
             FROM vec_chunks v
             JOIN chunks c ON v.chunk_id = c.id
             WHERE v.embedding MATCH :embedding AND k = :limit
             ORDER BY v.distance",
        )
        .context(DatabaseSnafu)?;

    stmt.query_map(
        rusqlite::named_params! {
            ":embedding": embedding_blob,
            ":limit": limit,
        },
        |row| {
            let chunk = indexed_chunk_from_row(row)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            let distance: f32 = row.get(8)?;
            let similarity = 1.0 - distance;
            Ok((chunk, similarity))
        },
    )
    .context(DatabaseSnafu)?
    .collect::<Result<Vec<_>, _>>()
    .context(DatabaseSnafu)
}
