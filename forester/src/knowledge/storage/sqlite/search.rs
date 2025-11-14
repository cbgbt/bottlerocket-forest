//! Search implementations for semantic and BM25 search

use rusqlite::Connection;
use snafu::ResultExt;

use super::serialization::{chunk_from_row, serialize_embedding};
use crate::knowledge::domain::Chunk;
use crate::knowledge::storage::repository::{StorageError, storage_error::*};

/// BM25 term saturation parameter (k1).
/// Controls how quickly term frequency saturates. Higher values (e.g., 2.0) give more weight
/// to repeated terms, lower values (e.g., 1.0) saturate faster. Typical range: 1.2-2.0.
/// See: https://en.wikipedia.org/wiki/Okapi_BM25
const BM25_K1: f64 = 1.2;

/// BM25 length normalization parameter (b).
/// Controls how much document length affects scoring. b=1.0 fully normalizes by length,
/// b=0.0 disables normalization. Typical range: 0.5-0.8.
/// See: https://en.wikipedia.org/wiki/Okapi_BM25
const BM25_B: f64 = 0.75;

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
            let chunk = chunk_from_row(row)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            let distance: f32 = row.get(13)?;
            let similarity = 1.0 - distance;
            Ok((chunk, similarity))
        },
    )
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

    let k1_plus_1 = BM25_K1 + 1.0;
    let one_minus_b = 1.0 - BM25_B;

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
                    -- TF component with BM25 normalization
                    (CAST(json_extract(c.bm25_terms, '$.' || ts.term) AS REAL) * {}) /
                    (CAST(json_extract(c.bm25_terms, '$.' || ts.term) AS REAL) + 
                     {} * ({} + {} * ((SELECT SUM(value) FROM json_each(c.bm25_terms)) / cs.avg_doc_length)))
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
            .join(","),
        k1_plus_1,
        BM25_K1,
        one_minus_b,
        BM25_B
    );

    let mut stmt = conn.prepare(&query).context(DatabaseSnafu)?;

    let mut params: Vec<&dyn rusqlite::ToSql> = query_terms
        .iter()
        .map(|t| t as &dyn rusqlite::ToSql)
        .collect();
    params.push(&limit);

    stmt.query_map(params.as_slice(), |row| {
        let chunk = chunk_from_row(row)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let score: f32 = row.get(13)?;
        Ok((chunk, score))
    })
    .context(DatabaseSnafu)?
    .collect::<Result<Vec<_>, _>>()
    .context(DatabaseSnafu)
}
