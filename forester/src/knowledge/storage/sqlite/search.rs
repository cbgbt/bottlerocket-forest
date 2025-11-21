//! Search implementation for semantic search

use rusqlite::Connection;
use snafu::ResultExt;

use super::serialization::{indexed_chunk_from_row, serialize_embedding};
use crate::knowledge::domain::IndexedChunk;
use crate::knowledge::storage::repository::{StorageError, storage_error::*};

/// Perform semantic search using sqlite-vec
///
/// Uses cosine distance for similarity measurement. For normalized vectors, cosine distance
/// equals `1 - cosine_similarity`, so lower distances indicate higher similarity.
/// We convert distance to similarity score via `1.0 - distance` for intuitive ranking.
///
/// Note: If embeddings are not normalized, the distance-to-similarity conversion may produce
/// values outside [0, 1]. The fastembed library used for embedding generation produces
/// normalized vectors by default.
///
/// The MATCH clause with `k = :limit` performs k-nearest-neighbor search, returning
/// the top k most similar vectors. See: https://github.com/asg017/sqlite-vec
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
