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
use crate::knowledge::domain::{ContextId, IndexedChunk};
use crate::knowledge::storage::repository::StorageError;

/// Performs k-nearest-neighbor search using cosine distance
///
/// Returns chunks ranked by similarity score in descending order. Similarity scores
/// are computed as `1.0 - distance` where distance is the cosine distance between
/// normalized embeddings.
///
/// When context_id is Some, results are filtered to files in that context via join
/// through indexed_files table. When context_id is None, all chunks are searched.
pub fn search_semantic(
    conn: &Connection,
    query_embedding: &[f32],
    limit: usize,
    context_id: Option<ContextId>,
) -> Result<Vec<(IndexedChunk, f32)>, StorageError> {
    use crate::knowledge::storage::repository::storage_error::*;

    let embedding_bytes = serialize_embedding(query_embedding);

    let (query, params): (String, Vec<Box<dyn rusqlite::ToSql>>) = match context_id {
        None => {
            let q = r#"
                SELECT c.chunk_hash, c.chunk_hash, c.file_hash, '', c.repo_name,
                    c.context_type, c.context_data, c.content, c.token_count, c.last_modified,
                    v.distance
                FROM vec_chunks v
                JOIN chunks c ON lower(hex(c.chunk_hash)) = v.chunk_hash
                WHERE v.embedding MATCH ? AND k = ?
                ORDER BY v.distance
            "#;
            (
                q.to_string(),
                vec![Box::new(embedding_bytes), Box::new(limit as i64)],
            )
        }
        Some(ctx) => {
            let q = r#"
                SELECT c.chunk_hash, c.chunk_hash, c.file_hash, '', c.repo_name,
                    c.context_type, c.context_data, c.content, c.token_count, c.last_modified,
                    v.distance
                FROM vec_chunks v
                JOIN chunks c ON lower(hex(c.chunk_hash)) = v.chunk_hash
                JOIN indexed_files i ON lower(hex(c.file_hash)) = lower(hex(i.file_hash))
                WHERE v.embedding MATCH ? AND k = ? AND i.context_id = ?
                ORDER BY v.distance
            "#;
            (
                q.to_string(),
                vec![
                    Box::new(embedding_bytes),
                    Box::new(limit as i64),
                    Box::new(ctx.to_string()),
                ],
            )
        }
    };

    let mut stmt = conn.prepare(&query).context(DatabaseSnafu)?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(params.iter()), |row| {
            let distance: f32 = row.get(10)?;
            let chunk = indexed_chunk_from_row(row).map_err(|_| rusqlite::Error::InvalidQuery)?;
            Ok((chunk, distance))
        })
        .context(DatabaseSnafu)?;

    let mut results = Vec::new();
    for row_result in rows {
        let (chunk, distance) = row_result.context(DatabaseSnafu)?;
        let similarity = 1.0 - distance;
        results.push((chunk, similarity));
    }

    Ok(results)
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
