//! Search implementations for semantic and BM25 search

use rusqlite::{Connection, params};
use snafu::ResultExt;

use super::serialization::{chunk_from_row, serialize_embedding};
use crate::knowledge::domain::Chunk;
use crate::knowledge::storage::repository::{StorageError, storage_error::*};

/// Perform semantic search using sqlite-vec
pub fn search_semantic(
    conn: &Connection,
    query_embedding: &[f32],
    limit: usize,
) -> Result<Vec<(Chunk, f32)>, StorageError> {
    let embedding_blob = serialize_embedding(query_embedding);

    let mut stmt = conn
        .prepare(
            "SELECT c.id, c.file_path, c.repo_name, c.line_start, c.line_count,
                    c.context_type, c.context_data, c.content, c.token_count, c.last_modified,
                    c.bm25_terms, c.index_mode, v.embedding, v.distance
             FROM vec_chunks v
             JOIN chunks c ON v.chunk_id = c.id
             WHERE v.embedding MATCH ?1 AND k = ?2
             ORDER BY v.distance",
        )
        .context(DatabaseSnafu)?;

    stmt.query_map(params![embedding_blob, limit], |row| {
        let chunk = chunk_from_row!(row)?;
        let distance: f32 = row.get(13)?;
        let similarity = 1.0 - distance;
        Ok((chunk, similarity))
    })
    .context(DatabaseSnafu)?
    .collect::<Result<Vec<_>, _>>()
    .context(DatabaseSnafu)
}

/// Perform BM25 search using SQL-based scoring
pub fn search_bm25(
    conn: &Connection,
    query_terms: &[String],
    limit: usize,
) -> Result<Vec<(Chunk, f32)>, StorageError> {
    if query_terms.is_empty() {
        return Ok(vec![]);
    }

    let query = format!(
        r#"
        WITH corpus_stats AS (
            SELECT 
                COUNT(*) as total_docs,
                AVG((SELECT SUM(value) FROM json_each(bm25_terms))) as avg_doc_length
            FROM chunks
            WHERE bm25_terms IS NOT NULL
        ),
        term_stats AS (
            SELECT 
                key as term,
                COUNT(*) as doc_freq
            FROM chunks, json_each(bm25_terms)
            WHERE bm25_terms IS NOT NULL
                AND key IN ({})
            GROUP BY key
        ),
        scored_docs AS (
            SELECT 
                c.id,
                c.file_path,
                c.repo_name,
                c.line_start,
                c.line_count,
                c.context_type,
                c.context_data,
                c.content,
                c.token_count,
                c.last_modified,
                c.bm25_terms,
                c.index_mode,
                NULL as embedding,
                SUM(
                    -- IDF component
                    LN((cs.total_docs - COALESCE(ts.doc_freq, 0) + 0.5) / (COALESCE(ts.doc_freq, 0) + 0.5) + 1.0) *
                    -- TF component with BM25 normalization (k1=1.2, b=0.75)
                    (CAST(json_extract(c.bm25_terms, '$.' || ts.term) AS REAL) * 2.2) /
                    (CAST(json_extract(c.bm25_terms, '$.' || ts.term) AS REAL) + 
                     1.2 * (0.25 + 0.75 * ((SELECT SUM(value) FROM json_each(c.bm25_terms)) / cs.avg_doc_length)))
                ) as score
            FROM chunks c
            CROSS JOIN corpus_stats cs
            LEFT JOIN term_stats ts ON json_extract(c.bm25_terms, '$.' || ts.term) IS NOT NULL
            WHERE c.bm25_terms IS NOT NULL
                AND ts.term IS NOT NULL
            GROUP BY c.id
            HAVING score > 0
            ORDER BY score DESC
            LIMIT ?
        )
        SELECT * FROM scored_docs
        "#,
        query_terms
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",")
    );

    let mut stmt = conn.prepare(&query).context(DatabaseSnafu)?;

    let mut params: Vec<&dyn rusqlite::ToSql> = query_terms
        .iter()
        .map(|t| t as &dyn rusqlite::ToSql)
        .collect();
    params.push(&limit);

    stmt.query_map(params.as_slice(), |row| {
        let chunk = chunk_from_row!(row)?;
        let score: f32 = row.get(13)?;
        Ok((chunk, score))
    })
    .context(DatabaseSnafu)?
    .collect::<Result<Vec<_>, _>>()
    .context(DatabaseSnafu)
}
