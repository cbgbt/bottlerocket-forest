//! Storage queries for content-addressed chunks.
//!
//! Provides CRUD operations for Chunk records using chunk_hash as the
//! primary key for content-addressed storage. This enables deduplication
//! across contexts - identical content shares embeddings regardless of
//! which context it appears in.

use rusqlite::Connection;
use snafu::{ResultExt, Snafu};

use crate::knowledge::constants::EMBEDDING_DIM;
use crate::knowledge::domain::{
    Chunk, ChunkContent, ChunkContext, ChunkHash, ChunkId, ChunkSource, Embedding, FileHash,
    ForestRelativePath, IndexedChunk, MarkdownContext, RepoName, RustDocContext, Timestamp,
    TokenCount,
};

/// Errors that can occur during chunk storage operations.
#[derive(Debug, Snafu)]
#[snafu(module, visibility(pub))]
pub enum ChunkStorageError {
    /// Database operation failed.
    #[snafu(display("Database operation failed"))]
    Database { source: rusqlite::Error },

    /// Invalid data retrieved from database.
    #[snafu(display("Invalid data in database: {message}"))]
    InvalidData { message: String },
}

/// Reconstructs an IndexedChunk from a database row with the new schema.
///
/// Expects columns: chunk_hash, file_hash, repo_name, context_type, context_data,
/// content, token_count, last_modified.
fn indexed_chunk_from_row(row: &rusqlite::Row) -> Result<IndexedChunk, ChunkStorageError> {
    use chunk_storage_error::*;

    let chunk_hash_bytes: Vec<u8> = row.get(0).context(DatabaseSnafu)?;
    let file_hash_bytes: Vec<u8> = row.get(1).context(DatabaseSnafu)?;
    let repo_name: String = row.get(2).context(DatabaseSnafu)?;
    let context_type: String = row.get(3).context(DatabaseSnafu)?;
    let context_data: String = row.get(4).context(DatabaseSnafu)?;
    let content: String = row.get(5).context(DatabaseSnafu)?;
    let token_count: i64 = row.get(6).context(DatabaseSnafu)?;
    let last_modified: i64 = row.get(7).context(DatabaseSnafu)?;

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

    let embedding = Embedding::try_new(vec![0.1; EMBEDDING_DIM])
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync + 'static>)
        .map_err(|_| {
            InvalidDataSnafu {
                message: "failed to create dummy embedding".to_string(),
            }
            .build()
        })?;

    let chunk = Chunk::builder()
        .id(ChunkId::new(uuid::Uuid::new_v4()))
        .chunk_hash(ChunkHash::new(chunk_hash_array))
        .file_hash(FileHash::new(file_hash_array))
        .source(
            ChunkSource::builder()
                .file_path(
                    ForestRelativePath::try_new("unknown.md")
                        .map_err(|e| {
                            Box::new(e) as Box<dyn std::error::Error + Send + Sync + 'static>
                        })
                        .map_err(|_| {
                            InvalidDataSnafu {
                                message: "failed to create file path".to_string(),
                            }
                            .build()
                        })?,
                )
                .repo_name(
                    RepoName::try_new(repo_name)
                        .map_err(|e| {
                            Box::new(e) as Box<dyn std::error::Error + Send + Sync + 'static>
                        })
                        .map_err(|_| {
                            InvalidDataSnafu {
                                message: "invalid repo_name".to_string(),
                            }
                            .build()
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
                        .map_err(|_| {
                            InvalidDataSnafu {
                                message: "invalid token_count".to_string(),
                            }
                            .build()
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

/// Converts ChunkContext to database-storable type and JSON representation
fn serialize_context(context: &ChunkContext) -> Result<(String, String), ChunkStorageError> {
    use chunk_storage_error::*;

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
fn deserialize_context(
    context_type: &str,
    context_data: &str,
) -> Result<ChunkContext, ChunkStorageError> {
    use chunk_storage_error::*;

    match context_type {
        "markdown" => {
            let ctx: MarkdownContext = serde_json::from_str(context_data).map_err(|e| {
                InvalidDataSnafu {
                    message: e.to_string(),
                }
                .build()
            })?;
            Ok(ChunkContext::Markdown(ctx))
        }
        "rust_doc" => {
            let ctx: RustDocContext = serde_json::from_str(context_data).map_err(|e| {
                InvalidDataSnafu {
                    message: e.to_string(),
                }
                .build()
            })?;
            Ok(ChunkContext::RustDoc(ctx))
        }
        _ => Err(InvalidDataSnafu {
            message: format!("unknown context type: {}", context_type),
        }
        .build()),
    }
}

/// Saves a chunk to storage (idempotent - handles UNIQUE constraint).
pub fn save_chunk(conn: &Connection, chunk: &IndexedChunk) -> Result<(), ChunkStorageError> {
    use chunk_storage_error::*;

    let (context_type, context_data) = serialize_context(&chunk.chunk.context)?;

    conn.execute(
        "INSERT OR REPLACE INTO chunks 
        (chunk_hash, file_hash, repo_name, context_type, context_data, content, token_count, last_modified)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            chunk.chunk.chunk_hash.as_bytes(),
            chunk.chunk.file_hash.as_bytes(),
            chunk.chunk.source.repo_name.to_string(),
            context_type,
            context_data,
            chunk.chunk.content.text,
            chunk.chunk.content.token_count.into_inner() as i64,
            chunk.indexed_at.as_secs(),
        ],
    )
    .context(DatabaseSnafu)?;

    Ok(())
}

/// Retrieves all chunks associated with a file hash.
pub fn get_chunks_by_file_hash(
    conn: &Connection,
    file_hash: &FileHash,
) -> Result<Vec<IndexedChunk>, ChunkStorageError> {
    use chunk_storage_error::*;

    let mut stmt = conn
        .prepare(
            "SELECT chunk_hash, file_hash, repo_name, context_type, context_data, 
                    content, token_count, last_modified
             FROM chunks
             WHERE file_hash = ?1",
        )
        .context(DatabaseSnafu)?;

    stmt.query_map([file_hash.as_bytes()], |row| {
        indexed_chunk_from_row(row)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
    })
    .context(DatabaseSnafu)?
    .collect::<Result<Vec<_>, _>>()
    .context(DatabaseSnafu)
}

/// Checks if a chunk with the given hash exists.
pub fn has_chunk(conn: &Connection, chunk_hash: &ChunkHash) -> Result<bool, ChunkStorageError> {
    use chunk_storage_error::*;

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM chunks WHERE chunk_hash = ?1",
            [chunk_hash.as_bytes()],
            |row| row.get(0),
        )
        .context(DatabaseSnafu)?;

    Ok(count > 0)
}

/// Deletes chunks not referenced by any context's indexed files.
///
/// Removes orphaned chunks whose file_hash is not present in the indexed_files table.
/// Also removes associated embeddings. Returns the count of deleted chunks.
pub fn delete_orphaned_chunks(conn: &Connection) -> Result<u64, ChunkStorageError> {
    use chunk_storage_error::*;

    let deleted_chunks = conn
        .execute(
            "DELETE FROM chunks WHERE file_hash NOT IN (SELECT DISTINCT file_hash FROM indexed_files)",
            [],
        )
        .context(DatabaseSnafu)?;

    conn.execute(
        "DELETE FROM vec_chunks WHERE chunk_hash NOT IN (SELECT chunk_hash FROM chunks)",
        [],
    )
    .context(DatabaseSnafu)?;

    Ok(deleted_chunks as u64)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::knowledge::domain::{
        Chunk, ChunkContent, ChunkContext, ChunkId, ChunkSource, Embedding, EmbeddingModelConfig,
        ForestRelativePath, HeadingText, MarkdownContext, RepoName, Timestamp, TokenCount,
    };
    use crate::knowledge::storage::schema;
    use rusqlite::Connection;
    use std::io::Cursor;
    use uuid::Uuid;

    fn setup_connection() -> Connection {
        // SAFETY: This call satisfies the safety requirements for sqlite3_auto_extension:
        // 1. We are not calling this from within an auto-extension handler
        // 2. We will not close any database connection from within the auto-extension
        // 3. We will not manipulate the auto-extension list from within an auto-extension
        // 4. sqlite3_vec_init is a valid C function pointer provided by the sqlite-vec crate
        // 5. The transmute is valid because both types are function pointers with the same size
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

    fn make_file_hash(content: &[u8]) -> FileHash {
        FileHash::from_reader(Cursor::new(content)).unwrap()
    }

    fn make_chunk(text: &str) -> Chunk {
        Chunk::builder()
            .id(ChunkId::new(Uuid::new_v4()))
            .chunk_hash(ChunkHash::from_text(text))
            .file_hash(make_file_hash(text.as_bytes()))
            .source(
                ChunkSource::builder()
                    .file_path(ForestRelativePath::try_new("test.md").unwrap())
                    .repo_name(RepoName::try_new("test-repo").unwrap())
                    .build(),
            )
            .content(
                ChunkContent::builder()
                    .text(text)
                    .token_count(TokenCount::try_new(10).unwrap())
                    .build(),
            )
            .context(ChunkContext::Markdown(
                MarkdownContext::builder()
                    .heading_hierarchy(vec![HeadingText::try_new("Test").unwrap()])
                    .build(),
            ))
            .build()
    }

    fn make_indexed_chunk(text: &str) -> IndexedChunk {
        IndexedChunk::builder()
            .chunk(make_chunk(text))
            .embedding(Embedding::try_new(vec![0.1, 0.2, 0.3]).unwrap())
            .indexed_at(Timestamp::from_secs(1700000000))
            .build()
    }

    fn make_indexed_chunk_with_file_hash(text: &str, file_hash: FileHash) -> IndexedChunk {
        let chunk = Chunk::builder()
            .id(ChunkId::new(Uuid::new_v4()))
            .chunk_hash(ChunkHash::from_text(text))
            .file_hash(file_hash)
            .source(
                ChunkSource::builder()
                    .file_path(ForestRelativePath::try_new("test.md").unwrap())
                    .repo_name(RepoName::try_new("test-repo").unwrap())
                    .build(),
            )
            .content(
                ChunkContent::builder()
                    .text(text)
                    .token_count(TokenCount::try_new(10).unwrap())
                    .build(),
            )
            .context(ChunkContext::Markdown(
                MarkdownContext::builder()
                    .heading_hierarchy(vec![HeadingText::try_new("Test").unwrap()])
                    .build(),
            ))
            .build();

        IndexedChunk::builder()
            .chunk(chunk)
            .embedding(Embedding::try_new(vec![0.1, 0.2, 0.3]).unwrap())
            .indexed_at(Timestamp::from_secs(1700000000))
            .build()
    }

    #[test]
    fn save_chunk_and_retrieve_by_file_hash() {
        // Given a database and an indexed chunk
        let conn = setup_connection();
        let indexed_chunk = make_indexed_chunk("This is test content");
        let file_hash = indexed_chunk.chunk.file_hash;

        // When saving the chunk
        save_chunk(&conn, &indexed_chunk).unwrap();

        // Then it should be retrievable by file_hash
        let chunks = get_chunks_by_file_hash(&conn, &file_hash).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].chunk.chunk_hash, indexed_chunk.chunk.chunk_hash);
        assert_eq!(chunks[0].chunk.content.text, "This is test content");
    }

    #[test]
    fn save_duplicate_chunk_is_idempotent() {
        // Given a database with an existing chunk
        let conn = setup_connection();
        let indexed_chunk = make_indexed_chunk("Duplicate content");

        // When saving the same chunk twice
        save_chunk(&conn, &indexed_chunk).unwrap();
        let result = save_chunk(&conn, &indexed_chunk);

        // Then the second save should succeed (idempotent)
        assert!(result.is_ok());

        // And only one chunk should exist
        let chunks = get_chunks_by_file_hash(&conn, &indexed_chunk.chunk.file_hash).unwrap();
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn get_chunks_by_file_hash_returns_correct_set() {
        // Given a database with multiple chunks from the same file
        let conn = setup_connection();
        let file_hash = make_file_hash(b"shared file content");

        let chunk1 = make_indexed_chunk_with_file_hash("First chunk of the file", file_hash);
        let chunk2 = make_indexed_chunk_with_file_hash("Second chunk of the file", file_hash);
        let chunk3 = make_indexed_chunk_with_file_hash("Third chunk of the file", file_hash);

        // And a chunk from a different file
        let other_file_hash = make_file_hash(b"different file");
        let other_chunk =
            make_indexed_chunk_with_file_hash("Chunk from other file", other_file_hash);

        save_chunk(&conn, &chunk1).unwrap();
        save_chunk(&conn, &chunk2).unwrap();
        save_chunk(&conn, &chunk3).unwrap();
        save_chunk(&conn, &other_chunk).unwrap();

        // When retrieving chunks by file_hash
        let chunks = get_chunks_by_file_hash(&conn, &file_hash).unwrap();

        // Then only chunks from that file should be returned
        assert_eq!(chunks.len(), 3);
        let texts: Vec<_> = chunks
            .iter()
            .map(|c| c.chunk.content.text.as_str())
            .collect();
        assert!(texts.contains(&"First chunk of the file"));
        assert!(texts.contains(&"Second chunk of the file"));
        assert!(texts.contains(&"Third chunk of the file"));
        assert!(!texts.contains(&"Chunk from other file"));
    }

    #[test]
    fn has_chunk_returns_true_for_existing() {
        // Given a database with a saved chunk
        let conn = setup_connection();
        let indexed_chunk = make_indexed_chunk("Existing chunk content");
        save_chunk(&conn, &indexed_chunk).unwrap();

        // When checking if the chunk exists
        let exists = has_chunk(&conn, &indexed_chunk.chunk.chunk_hash).unwrap();

        // Then it should return true
        assert!(exists);
    }

    #[test]
    fn has_chunk_returns_false_for_missing() {
        // Given a database with no chunks
        let conn = setup_connection();
        let nonexistent_hash = ChunkHash::from_text("content that was never saved");

        // When checking if a non-existent chunk exists
        let exists = has_chunk(&conn, &nonexistent_hash).unwrap();

        // Then it should return false
        assert!(!exists);
    }

    #[test]
    fn delete_orphaned_chunks_returns_zero_on_empty_database() {
        // Given an empty database with no chunks
        let conn = setup_connection();

        // When running garbage collection
        let result = delete_orphaned_chunks(&conn);

        // Then it should return zero deleted chunks
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn delete_orphaned_chunks_returns_zero_when_all_chunks_referenced() {
        // Given a database with chunks that are all referenced by indexed_files
        let conn = setup_connection();
        let indexed_chunk = make_indexed_chunk("Referenced content");
        save_chunk(&conn, &indexed_chunk).unwrap();

        // And the chunk's file_hash is in indexed_files
        conn.execute(
            "INSERT INTO indexed_files (context_id, file_path, file_hash, mtime_ns) VALUES (?, ?, ?, ?)",
            rusqlite::params![
                ".",
                "test.md",
                indexed_chunk.chunk.file_hash.as_bytes().as_slice(),
                1700000000_i64 * 1_000_000_000,
            ],
        ).unwrap();

        // When running garbage collection
        let result = delete_orphaned_chunks(&conn);

        // Then it should return zero (no orphans)
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);

        // And the chunk should still exist
        assert!(has_chunk(&conn, &indexed_chunk.chunk.chunk_hash).unwrap());
    }

    #[test]
    fn delete_orphaned_chunks_deletes_unreferenced_chunks() {
        // Given a database with a chunk not referenced by any indexed_files
        let conn = setup_connection();
        let orphan_chunk = make_indexed_chunk("Orphaned content");
        save_chunk(&conn, &orphan_chunk).unwrap();

        // And no entry in indexed_files for this file_hash

        // When running garbage collection
        let result = delete_orphaned_chunks(&conn);

        // Then it should return 1 (one orphan deleted)
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);

        // And the chunk should no longer exist
        assert!(!has_chunk(&conn, &orphan_chunk.chunk.chunk_hash).unwrap());
    }

    #[test]
    fn delete_orphaned_chunks_preserves_shared_content() {
        // Given a database with a chunk referenced by one context
        let conn = setup_connection();
        let file_hash = make_file_hash(b"shared file content");
        let chunk = make_indexed_chunk_with_file_hash("Shared content", file_hash);
        save_chunk(&conn, &chunk).unwrap();

        // And two contexts reference the same file_hash
        conn.execute(
            "INSERT INTO indexed_files (context_id, file_path, file_hash, mtime_ns) VALUES (?, ?, ?, ?)",
            rusqlite::params!["context-a", "file.md", file_hash.as_bytes().as_slice(), 1700000000_i64 * 1_000_000_000],
        ).unwrap();
        conn.execute(
            "INSERT INTO indexed_files (context_id, file_path, file_hash, mtime_ns) VALUES (?, ?, ?, ?)",
            rusqlite::params!["context-b", "file.md", file_hash.as_bytes().as_slice(), 1700000000_i64 * 1_000_000_000],
        ).unwrap();

        // When removing one context's reference
        conn.execute(
            "DELETE FROM indexed_files WHERE context_id = ?",
            ["context-a"],
        )
        .unwrap();

        // And running garbage collection
        let result = delete_orphaned_chunks(&conn);

        // Then it should return zero (chunk still referenced by context-b)
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);

        // And the chunk should still exist
        assert!(has_chunk(&conn, &chunk.chunk.chunk_hash).unwrap());
    }

    #[test]
    fn delete_orphaned_chunks_deletes_after_all_references_removed() {
        // Given a database with a chunk referenced by two contexts
        let conn = setup_connection();
        let file_hash = make_file_hash(b"shared file content for deletion");
        let chunk = make_indexed_chunk_with_file_hash("Content to be orphaned", file_hash);
        save_chunk(&conn, &chunk).unwrap();

        conn.execute(
            "INSERT INTO indexed_files (context_id, file_path, file_hash, mtime_ns) VALUES (?, ?, ?, ?)",
            rusqlite::params!["context-a", "file.md", file_hash.as_bytes().as_slice(), 1700000000_i64 * 1_000_000_000],
        ).unwrap();
        conn.execute(
            "INSERT INTO indexed_files (context_id, file_path, file_hash, mtime_ns) VALUES (?, ?, ?, ?)",
            rusqlite::params!["context-b", "file.md", file_hash.as_bytes().as_slice(), 1700000000_i64 * 1_000_000_000],
        ).unwrap();

        // When removing all references
        conn.execute("DELETE FROM indexed_files", []).unwrap();

        // And running garbage collection
        let result = delete_orphaned_chunks(&conn);

        // Then it should return 1 (orphan deleted)
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);

        // And the chunk should no longer exist
        assert!(!has_chunk(&conn, &chunk.chunk.chunk_hash).unwrap());
    }

    #[test]
    fn delete_orphaned_chunks_returns_correct_count_for_multiple_orphans() {
        // Given a database with multiple orphaned chunks
        let conn = setup_connection();
        let chunk1 = make_indexed_chunk("Orphan 1");
        let chunk2 =
            make_indexed_chunk_with_file_hash("Orphan 2", make_file_hash(b"different file"));
        let chunk3 = make_indexed_chunk_with_file_hash("Orphan 3", make_file_hash(b"another file"));
        save_chunk(&conn, &chunk1).unwrap();
        save_chunk(&conn, &chunk2).unwrap();
        save_chunk(&conn, &chunk3).unwrap();

        // When running garbage collection
        let result = delete_orphaned_chunks(&conn);

        // Then it should return 3 (all orphans deleted)
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 3);
    }
}
