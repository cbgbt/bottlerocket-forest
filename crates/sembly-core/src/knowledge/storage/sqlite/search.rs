//! Semantic search implementation using vector embeddings
//!
//! Provides k-nearest-neighbor search using sqlite-vec with cosine distance.
//!
//! # Implementation Notes
//!
//! Similarity scores are computed as `1.0 - distance` where distance is cosine distance.
//! This assumes normalized embeddings (as produced by fastembed). Non-normalized embeddings
//! may produce scores outside [0, 1].
//!
//! # Context Filtering
//!
//! sqlite-vec's `k` parameter limits results BEFORE any JOIN/WHERE filters are applied.
//! To properly filter by context, we use a two-phase approach:
//! 1. First, get the set of valid file_hashes for the context
//! 2. Then, request extra results from KNN (multiplier of limit) and filter in application code
//!
//! This ensures context isolation is maintained (MCI-15).

use rusqlite::Connection;
use snafu::ResultExt;
use std::collections::HashSet;

use super::serialization::{indexed_chunk_from_row, serialize_embedding};
use crate::knowledge::domain::{ContextId, IndexedChunk};
use crate::knowledge::storage::repository::StorageError;

/// Multiplier for KNN search when filtering by context.
/// We request more results than needed to account for filtering.
const CONTEXT_FILTER_MULTIPLIER: usize = 10;

/// Maximum k value for KNN queries (sqlite-vec default limit is 4096)
const MAX_KNN_K: usize = 4096;

/// Performs k-nearest-neighbor search using cosine distance
///
/// Returns chunks ranked by similarity score in descending order. Similarity scores
/// are computed as `1.0 - distance` where distance is the cosine distance between
/// normalized embeddings.
///
/// When context_id is Some, results are filtered to files in that context.
/// When context_id is None, all chunks are searched.
pub fn search_semantic(
    conn: &Connection,
    query_embedding: &[f32],
    limit: usize,
    context_id: Option<ContextId>,
) -> Result<Vec<(IndexedChunk, f32)>, StorageError> {
    use crate::knowledge::storage::repository::storage_error::*;

    let embedding_bytes = serialize_embedding(query_embedding);

    match context_id {
        None => {
            // No context filter - simple KNN query
            let query = r#"
                SELECT c.chunk_hash, c.chunk_hash, c.file_hash, '', c.repo_name,
                    c.context_type, c.context_data, c.content, c.token_count, c.last_modified,
                    v.distance
                FROM vec_chunks v
                JOIN chunks c ON lower(hex(c.chunk_hash)) = v.chunk_hash
                WHERE v.embedding MATCH ? AND k = ?
                ORDER BY v.distance
            "#;

            let mut stmt = conn.prepare(query).context(DatabaseSnafu)?;
            let rows = stmt
                .query_map(
                    [&embedding_bytes as &dyn rusqlite::ToSql, &(limit as i64)],
                    |row| {
                        let distance: f32 = row.get(10)?;
                        let chunk = indexed_chunk_from_row(row)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?;
                        Ok((chunk, distance))
                    },
                )
                .context(DatabaseSnafu)?;

            let mut results = Vec::new();
            for row_result in rows {
                let (chunk, distance) = row_result.context(DatabaseSnafu)?;
                let similarity = 1.0 - distance;
                results.push((chunk, similarity));
            }

            Ok(results)
        }
        Some(ctx) => {
            // Context filter - two-phase approach for proper isolation
            // Phase 1: Get valid file_hashes for this context
            let valid_file_hashes = get_context_file_hashes(conn, &ctx)?;

            if valid_file_hashes.is_empty() {
                return Ok(Vec::new());
            }

            // Phase 2: KNN search with extra results, then filter
            let knn_limit = (limit * CONTEXT_FILTER_MULTIPLIER).min(MAX_KNN_K);

            let query = r#"
                SELECT c.chunk_hash, c.chunk_hash, c.file_hash, '', c.repo_name,
                    c.context_type, c.context_data, c.content, c.token_count, c.last_modified,
                    v.distance
                FROM vec_chunks v
                JOIN chunks c ON lower(hex(c.chunk_hash)) = v.chunk_hash
                WHERE v.embedding MATCH ? AND k = ?
                ORDER BY v.distance
            "#;

            let mut stmt = conn.prepare(query).context(DatabaseSnafu)?;
            let rows = stmt
                .query_map(
                    [
                        &embedding_bytes as &dyn rusqlite::ToSql,
                        &(knn_limit as i64),
                    ],
                    |row| {
                        let distance: f32 = row.get(10)?;
                        let chunk = indexed_chunk_from_row(row)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?;
                        Ok((chunk, distance))
                    },
                )
                .context(DatabaseSnafu)?;

            // Filter results to only include chunks from valid files
            let mut results = Vec::new();
            for row_result in rows {
                let (chunk, distance) = row_result.context(DatabaseSnafu)?;

                // Check if this chunk's file_hash is in the context
                if valid_file_hashes.contains(&chunk.chunk.file_hash) {
                    let similarity = 1.0 - distance;
                    results.push((chunk, similarity));

                    if results.len() >= limit {
                        break;
                    }
                }
            }

            Ok(results)
        }
    }
}

/// Gets the set of file hashes that belong to a specific context
fn get_context_file_hashes(
    conn: &Connection,
    context_id: &ContextId,
) -> Result<HashSet<crate::knowledge::domain::FileHash>, StorageError> {
    use crate::knowledge::storage::repository::storage_error::*;

    let query = "SELECT file_hash FROM indexed_files WHERE context_id = ?";
    let mut stmt = conn.prepare(query).context(DatabaseSnafu)?;

    let rows = stmt
        .query_map([context_id.to_string()], |row| {
            let hash_bytes: Vec<u8> = row.get(0)?;
            Ok(hash_bytes)
        })
        .context(DatabaseSnafu)?;

    let mut hashes = HashSet::new();
    for row_result in rows {
        let hash_bytes = row_result.context(DatabaseSnafu)?;
        if hash_bytes.len() == 32 {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&hash_bytes);
            hashes.insert(crate::knowledge::domain::FileHash::new(arr));
        }
    }

    Ok(hashes)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::knowledge::constants::EMBEDDING_DIM;
    use crate::knowledge::domain::{
        Chunk, ChunkContent, ChunkContext, ChunkHash, ChunkId, ChunkSource, Embedding,
        EmbeddingModelConfig, FileHash, ForestRelativePath, IndexedFile, MarkdownContext, RepoName,
        Timestamp, TokenCount,
    };
    use crate::knowledge::storage::schema;
    use crate::knowledge::storage::sqlite::files::insert_indexed_file;
    use rusqlite::Connection;

    fn setup_connection() -> Connection {
        #[allow(clippy::missing_transmute_annotations)]
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }
        let conn = Connection::open_in_memory().unwrap();
        schema::create_tables(&conn, &EmbeddingModelConfig::default()).unwrap();
        conn
    }

    fn create_chunk_with_embedding(
        content: &str,
        file_hash: FileHash,
        embedding_values: Vec<f32>,
    ) -> IndexedChunk {
        let chunk_hash = ChunkHash::from_text(content);
        let chunk = Chunk::builder()
            .id(ChunkId::new(uuid::Uuid::new_v4()))
            .chunk_hash(chunk_hash)
            .file_hash(file_hash)
            .source(
                ChunkSource::builder()
                    .file_path(ForestRelativePath::try_new("test.md").unwrap())
                    .repo_name(RepoName::try_new("test-repo").unwrap())
                    .build(),
            )
            .content(
                ChunkContent::builder()
                    .text(content)
                    .token_count(TokenCount::try_new(10).unwrap())
                    .build(),
            )
            .context(ChunkContext::Markdown(
                MarkdownContext::builder().heading_hierarchy(vec![]).build(),
            ))
            .build();

        IndexedChunk::builder()
            .chunk(chunk)
            .embedding(Embedding::try_new(embedding_values).unwrap())
            .indexed_at(Timestamp::now())
            .build()
    }

    fn insert_chunk_and_embedding(conn: &Connection, chunk: &IndexedChunk) {
        use crate::knowledge::storage::sqlite::chunks::save_chunk;
        save_chunk(conn, chunk).unwrap();

        let embedding_bytes: Vec<u8> = chunk
            .embedding
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();
        conn.execute(
            "INSERT INTO vec_chunks (chunk_hash, embedding) VALUES (?, ?)",
            rusqlite::params![chunk.chunk.chunk_hash.to_string(), embedding_bytes],
        )
        .unwrap();
    }

    #[test]
    fn search_semantic_returns_all_chunks_when_context_is_none() {
        // Given a database with chunks in multiple contexts
        let conn = setup_connection();

        let file_hash_a = FileHash::new([1u8; 32]);
        let file_hash_b = FileHash::new([2u8; 32]);

        let chunk_a =
            create_chunk_with_embedding("content A", file_hash_a, vec![0.8; EMBEDDING_DIM]);
        let chunk_b =
            create_chunk_with_embedding("content B", file_hash_b, vec![0.7; EMBEDDING_DIM]);

        insert_chunk_and_embedding(&conn, &chunk_a);
        insert_chunk_and_embedding(&conn, &chunk_b);

        let context_a = ContextId::from_path("context-a").unwrap();
        let context_b = ContextId::from_path("context-b").unwrap();

        insert_indexed_file(
            &conn,
            &IndexedFile::builder()
                .context_id(context_a)
                .file_path(ForestRelativePath::try_new("file-a.md").unwrap())
                .file_hash(file_hash_a)
                .mtime_ns(1000)
                .build(),
        )
        .unwrap();

        insert_indexed_file(
            &conn,
            &IndexedFile::builder()
                .context_id(context_b)
                .file_path(ForestRelativePath::try_new("file-b.md").unwrap())
                .file_hash(file_hash_b)
                .mtime_ns(2000)
                .build(),
        )
        .unwrap();

        // When searching with context_id = None
        let query = vec![0.75; EMBEDDING_DIM];
        let results = search_semantic(&conn, &query, 10, None).unwrap();

        // Then all chunks should be returned
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn search_semantic_filters_by_context_when_provided() {
        // Given a database with chunks in multiple contexts
        let conn = setup_connection();

        let file_hash_a = FileHash::new([1u8; 32]);
        let file_hash_b = FileHash::new([2u8; 32]);

        let chunk_a = create_chunk_with_embedding(
            "content in context A",
            file_hash_a,
            vec![0.8; EMBEDDING_DIM],
        );
        let chunk_b = create_chunk_with_embedding(
            "content in context B",
            file_hash_b,
            vec![0.7; EMBEDDING_DIM],
        );

        insert_chunk_and_embedding(&conn, &chunk_a);
        insert_chunk_and_embedding(&conn, &chunk_b);

        let context_a = ContextId::from_path("context-a").unwrap();
        let context_b = ContextId::from_path("context-b").unwrap();

        insert_indexed_file(
            &conn,
            &IndexedFile::builder()
                .context_id(context_a.clone())
                .file_path(ForestRelativePath::try_new("file-a.md").unwrap())
                .file_hash(file_hash_a)
                .mtime_ns(1000)
                .build(),
        )
        .unwrap();

        insert_indexed_file(
            &conn,
            &IndexedFile::builder()
                .context_id(context_b)
                .file_path(ForestRelativePath::try_new("file-b.md").unwrap())
                .file_hash(file_hash_b)
                .mtime_ns(2000)
                .build(),
        )
        .unwrap();

        // When searching with context_id = Some(context_a)
        let query = vec![0.75; EMBEDDING_DIM];
        let results = search_semantic(&conn, &query, 10, Some(context_a)).unwrap();

        // Then only chunks from context_a should be returned
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.chunk.file_hash, file_hash_a);
    }

    #[test]
    fn search_semantic_returns_empty_when_context_has_no_files() {
        // Given a database with chunks but an empty context
        let conn = setup_connection();

        let file_hash = FileHash::new([1u8; 32]);
        let chunk =
            create_chunk_with_embedding("some content", file_hash, vec![0.8; EMBEDDING_DIM]);
        insert_chunk_and_embedding(&conn, &chunk);

        let populated_context = ContextId::from_path("populated").unwrap();
        let empty_context = ContextId::from_path("empty").unwrap();

        insert_indexed_file(
            &conn,
            &IndexedFile::builder()
                .context_id(populated_context)
                .file_path(ForestRelativePath::try_new("file.md").unwrap())
                .file_hash(file_hash)
                .mtime_ns(1000)
                .build(),
        )
        .unwrap();

        // When searching the empty context
        let query = vec![0.8; EMBEDDING_DIM];
        let results = search_semantic(&conn, &query, 10, Some(empty_context)).unwrap();

        // Then no results should be returned
        assert!(results.is_empty());
    }

    /// This test verifies that context filtering works correctly even when
    /// the target context has fewer matching chunks than the k limit.
    ///
    /// sqlite-vec's k parameter limits results BEFORE any WHERE clause filtering.
    /// A naive query like:
    ///   SELECT ... FROM vec_chunks v JOIN chunks c JOIN indexed_files i
    ///   WHERE v.embedding MATCH ? AND k = 2 AND i.context_id = 'target'
    ///
    /// Would first return the top 2 nearest neighbors (which might all be from
    /// other contexts), then filter by context_id, potentially returning 0 results
    /// even though the target context has matching content.
    ///
    /// Our implementation uses a two-phase approach to avoid this issue.
    #[test]
    fn search_semantic_finds_results_when_other_context_has_closer_matches() {
        // Given: context_a has 1 chunk, context_b has 5 chunks with closer embeddings
        let conn = setup_connection();

        // Create chunks for context_b with embeddings very close to query
        let file_hash_b = FileHash::new([2u8; 32]);
        for i in 0..5 {
            let mut embedding = vec![0.99; EMBEDDING_DIM];
            embedding[0] = 0.99 - (i as f32 * 0.001); // Slightly vary each one
            let chunk = create_chunk_with_embedding(
                &format!("context B chunk {}", i),
                file_hash_b,
                embedding,
            );
            insert_chunk_and_embedding(&conn, &chunk);
        }

        // Create chunk for context_a with embedding further from query
        let file_hash_a = FileHash::new([1u8; 32]);
        let chunk_a =
            create_chunk_with_embedding("context A content", file_hash_a, vec![0.5; EMBEDDING_DIM]);
        insert_chunk_and_embedding(&conn, &chunk_a);

        let context_a = ContextId::from_path("context-a").unwrap();
        let context_b = ContextId::from_path("context-b").unwrap();

        insert_indexed_file(
            &conn,
            &IndexedFile::builder()
                .context_id(context_a.clone())
                .file_path(ForestRelativePath::try_new("file-a.md").unwrap())
                .file_hash(file_hash_a)
                .mtime_ns(1000)
                .build(),
        )
        .unwrap();

        insert_indexed_file(
            &conn,
            &IndexedFile::builder()
                .context_id(context_b)
                .file_path(ForestRelativePath::try_new("file-b.md").unwrap())
                .file_hash(file_hash_b)
                .mtime_ns(2000)
                .build(),
        )
        .unwrap();

        // When: searching context_a with limit=2 and query close to context_b's embeddings
        // A naive k=2 query would return only context_b results, then filter to 0
        let query = vec![0.99; EMBEDDING_DIM];
        let results = search_semantic(&conn, &query, 2, Some(context_a)).unwrap();

        // Then: should still find context_a's chunk despite context_b having closer matches
        assert_eq!(
            results.len(),
            1,
            "Should find context_a's chunk even though context_b has closer matches"
        );
        assert_eq!(results[0].0.chunk.file_hash, file_hash_a);
    }

    #[test]
    fn search_semantic_does_not_leak_results_across_contexts() {
        // Given a database with chunks indexed only in context A
        let conn = setup_connection();

        let file_hash = FileHash::new([1u8; 32]);
        let chunk =
            create_chunk_with_embedding("secret content", file_hash, vec![0.9; EMBEDDING_DIM]);
        insert_chunk_and_embedding(&conn, &chunk);

        let context_a = ContextId::from_path("context-a").unwrap();
        let context_b = ContextId::from_path("context-b").unwrap();

        // File is only indexed in context_a
        insert_indexed_file(
            &conn,
            &IndexedFile::builder()
                .context_id(context_a)
                .file_path(ForestRelativePath::try_new("secret.md").unwrap())
                .file_hash(file_hash)
                .mtime_ns(1000)
                .build(),
        )
        .unwrap();

        // When searching context_b (which has no files)
        let query = vec![0.9; EMBEDDING_DIM];
        let results = search_semantic(&conn, &query, 10, Some(context_b)).unwrap();

        // Then results from context_a should NOT appear (MCI-15)
        assert!(
            results.is_empty(),
            "Chunks from context_a should not leak into context_b search results"
        );
    }
}
