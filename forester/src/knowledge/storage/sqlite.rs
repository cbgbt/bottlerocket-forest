//! SQLite implementation of ChunkRepository

use rusqlite::{Connection, OptionalExtension, params};
use snafu::ResultExt;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use super::repository::{ChunkRepository, IndexMetadata, StorageError};
use super::schema;
use crate::knowledge::domain::{Chunk, ChunkContext, ChunkId, ForestRelativePath, IndexMode};

/// SQLite-backed chunk repository
pub struct SqliteChunkRepository {
    conn: Connection,
}

impl SqliteChunkRepository {
    /// Open or create a database at the given path
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        use super::repository::storage_error::*;

        // Register sqlite-vec extension
        // SAFETY: This call is safe because:
        // - We are not opening a database from within the auto-extension handler
        // - We are not closing the database from within the auto-extension handler
        // - We are not manipulating the auto-extension list from within an auto-extension
        // - sqlite3_vec_init is a valid C function pointer provided by the sqlite-vec crate
        // - The transmute converts the function pointer to the type expected by sqlite3_auto_extension
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }

        let conn = Connection::open(path.as_ref()).context(DatabaseSnafu)?;

        schema::create_tables(&conn).map_err(|e| {
            InvalidDataSnafu {
                message: e.to_string(),
            }
            .build()
        })?;

        Ok(Self { conn })
    }

    fn serialize_embedding(embedding: &[f32]) -> Vec<u8> {
        embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
    }

    fn serialize_context(context: &ChunkContext) -> Result<(String, String), StorageError> {
        use super::repository::storage_error::*;

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

    fn deserialize_context(
        context_type: &str,
        context_data: &str,
    ) -> Result<ChunkContext, StorageError> {
        use super::repository::storage_error::*;

        match context_type {
            "markdown" => {
                let ctx = serde_json::from_str(context_data).context(SerializationSnafu)?;
                Ok(ChunkContext::Markdown(ctx))
            }
            "rust_doc" => {
                let ctx = serde_json::from_str(context_data).context(SerializationSnafu)?;
                Ok(ChunkContext::RustDoc(ctx))
            }
            _ => Err(InvalidDataSnafu {
                message: format!("unknown context type: {}", context_type),
            }
            .build()),
        }
    }

    fn system_time_to_unix(time: SystemTime) -> i64 {
        time.duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
    }

    fn unix_to_system_time(unix: i64) -> SystemTime {
        UNIX_EPOCH + std::time::Duration::from_secs(unix as u64)
    }

    fn deserialize_chunk(
        id_str: String,
        file_path: String,
        repo_name: String,
        line_start: i64,
        line_end: i64,
        context_type: String,
        context_data: String,
        content: String,
        last_modified: i64,
    ) -> Result<Chunk, StorageError> {
        use super::repository::storage_error::*;

        let uuid = uuid::Uuid::parse_str(&id_str).map_err(|e| {
            InvalidDataSnafu {
                message: e.to_string(),
            }
            .build()
        })?;

        Ok(Chunk::builder()
            .id(ChunkId::new(uuid))
            .source(
                crate::knowledge::domain::ChunkSource::builder()
                    .file_path(ForestRelativePath::try_new(file_path).map_err(|e| {
                        InvalidDataSnafu {
                            message: e.to_string(),
                        }
                        .build()
                    })?)
                    .repo_name(
                        crate::knowledge::domain::RepoName::try_new(repo_name).map_err(|e| {
                            InvalidDataSnafu {
                                message: e.to_string(),
                            }
                            .build()
                        })?,
                    )
                    .line_range(
                        crate::knowledge::domain::LineRange::builder()
                            .start(
                                crate::knowledge::domain::LineNumber::try_new(line_start as usize)
                                    .map_err(|e| {
                                        InvalidDataSnafu {
                                            message: e.to_string(),
                                        }
                                        .build()
                                    })?,
                            )
                            .line_count(
                                crate::knowledge::domain::LineCount::try_new(line_end as usize)
                                    .map_err(|e| {
                                        InvalidDataSnafu {
                                            message: e.to_string(),
                                        }
                                        .build()
                                    })?,
                            )
                            .build(),
                    )
                    .build(),
            )
            .content(
                crate::knowledge::domain::ChunkContent::builder()
                    .text(content.clone())
                    .token_count(
                        crate::knowledge::domain::TokenCount::try_new(content.len()).map_err(
                            |e| {
                                InvalidDataSnafu {
                                    message: e.to_string(),
                                }
                                .build()
                            },
                        )?,
                    )
                    .build(),
            )
            .context(Self::deserialize_context(&context_type, &context_data)?)
            .indexed_at(Self::unix_to_system_time(last_modified))
            .build())
    }
}

impl ChunkRepository for SqliteChunkRepository {
    fn save(&mut self, chunk: &Chunk) -> Result<(), StorageError> {
        use super::repository::storage_error::*;

        let (context_type, context_data) = Self::serialize_context(&chunk.context)?;

        self.conn
            .execute(
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
                    Self::system_time_to_unix(chunk.indexed_at),
                    "fast",
                ],
            )
            .context(DatabaseSnafu)?;

        Ok(())
    }

    fn save_batch(&mut self, chunks: &[Chunk]) -> Result<(), StorageError> {
        use super::repository::storage_error::*;

        let tx = self.conn.transaction().context(DatabaseSnafu)?;

        for chunk in chunks {
            let (context_type, context_data) = Self::serialize_context(&chunk.context)?;

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
                    Self::system_time_to_unix(chunk.indexed_at),
                    "fast",
                ],
            )
            .context(DatabaseSnafu)?;
        }

        tx.commit().context(DatabaseSnafu)?;

        Ok(())
    }

    fn find_by_id(&self, id: &ChunkId) -> Result<Option<Chunk>, StorageError> {
        use super::repository::storage_error::*;

        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, file_path, repo_name, line_start, line_end, 
                        context_type, context_data, content, last_modified
                 FROM chunks WHERE id = ?1",
            )
            .context(DatabaseSnafu)?;

        let chunk = stmt
            .query_row(params![id.to_string()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, i64>(8)?,
                ))
            })
            .optional()
            .context(DatabaseSnafu)?;

        chunk
            .map(
                |(
                    id_str,
                    file_path,
                    repo_name,
                    line_start,
                    line_end,
                    context_type,
                    context_data,
                    content,
                    last_modified,
                )| {
                    Self::deserialize_chunk(
                        id_str,
                        file_path,
                        repo_name,
                        line_start,
                        line_end,
                        context_type,
                        context_data,
                        content,
                        last_modified,
                    )
                },
            )
            .transpose()
    }

    fn find_by_file(&self, path: &ForestRelativePath) -> Result<Vec<Chunk>, StorageError> {
        use super::repository::storage_error::*;

        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, file_path, repo_name, line_start, line_end, 
                        context_type, context_data, content, last_modified
                 FROM chunks WHERE file_path = ?1",
            )
            .context(DatabaseSnafu)?;

        let rows = stmt
            .query_map(params![path.to_string()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, i64>(8)?,
                ))
            })
            .context(DatabaseSnafu)?;

        rows.map(|row| {
            let (
                id_str,
                file_path,
                repo_name,
                line_start,
                line_end,
                context_type,
                context_data,
                content,
                last_modified,
            ) = row.context(DatabaseSnafu)?;
            Self::deserialize_chunk(
                id_str,
                file_path,
                repo_name,
                line_start,
                line_end,
                context_type,
                context_data,
                content,
                last_modified,
            )
        })
        .collect()
    }

    fn find_all(&self) -> Result<Vec<Chunk>, StorageError> {
        use super::repository::storage_error::*;

        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, file_path, repo_name, line_start, line_end, 
                        context_type, context_data, content, last_modified
                 FROM chunks",
            )
            .context(DatabaseSnafu)?;

        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, i64>(8)?,
                ))
            })
            .context(DatabaseSnafu)?;

        rows.map(|row| {
            let (
                id_str,
                file_path,
                repo_name,
                line_start,
                line_end,
                context_type,
                context_data,
                content,
                last_modified,
            ) = row.context(DatabaseSnafu)?;
            Self::deserialize_chunk(
                id_str,
                file_path,
                repo_name,
                line_start,
                line_end,
                context_type,
                context_data,
                content,
                last_modified,
            )
        })
        .collect()
    }

    fn delete_by_file(&mut self, path: &ForestRelativePath) -> Result<usize, StorageError> {
        use super::repository::storage_error::*;

        let count = self
            .conn
            .execute(
                "DELETE FROM chunks WHERE file_path = ?1",
                params![path.to_string()],
            )
            .context(DatabaseSnafu)?;

        Ok(count)
    }

    fn clear(&mut self) -> Result<usize, StorageError> {
        use super::repository::storage_error::*;

        let count = self
            .conn
            .execute("DELETE FROM chunks", [])
            .context(DatabaseSnafu)?;

        Ok(count)
    }

    fn get_metadata(&self) -> Result<IndexMetadata, StorageError> {
        use super::repository::storage_error::*;

        let mode_str: String = self
            .conn
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

        let last_build_unix: i64 = self
            .conn
            .query_row(
                "SELECT value FROM index_metadata WHERE key = 'last_build'",
                [],
                |row| row.get(0),
            )
            .optional()
            .context(DatabaseSnafu)?
            .and_then(|s: String| s.parse().ok())
            .unwrap_or(0);

        let chunk_count: usize = self
            .conn
            .query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))
            .context(DatabaseSnafu)?;

        let file_count: usize = self
            .conn
            .query_row("SELECT COUNT(DISTINCT file_path) FROM chunks", [], |row| {
                row.get(0)
            })
            .context(DatabaseSnafu)?;

        Ok(IndexMetadata::builder()
            .mode(mode)
            .last_build(Self::unix_to_system_time(last_build_unix))
            .chunk_count(chunk_count)
            .file_count(file_count)
            .build())
    }

    fn set_metadata(&mut self, metadata: &IndexMetadata) -> Result<(), StorageError> {
        use super::repository::storage_error::*;

        self.conn
            .execute(
                "INSERT OR REPLACE INTO index_metadata (key, value) VALUES ('mode', ?1)",
                params![metadata.mode.to_string()],
            )
            .context(DatabaseSnafu)?;

        self.conn
            .execute(
                "INSERT OR REPLACE INTO index_metadata (key, value) VALUES ('last_build', ?1)",
                params![Self::system_time_to_unix(metadata.last_build).to_string()],
            )
            .context(DatabaseSnafu)?;

        Ok(())
    }

    fn search_semantic(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<(Chunk, f32)>, StorageError> {
        use super::repository::storage_error::*;

        let embedding_blob = Self::serialize_embedding(query_embedding);

        let mut stmt = self
            .conn
            .prepare(
                "SELECT c.id, c.file_path, c.repo_name, c.line_start, c.line_end,
                        c.context_type, c.context_data, c.content, c.last_modified,
                        v.distance
                 FROM vec_chunks v
                 JOIN chunks c ON v.chunk_id = c.id
                 WHERE v.embedding MATCH ?1 AND k = ?2
                 ORDER BY v.distance",
            )
            .context(DatabaseSnafu)?;

        let rows = stmt
            .query_map(params![embedding_blob, limit], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, f32>(9)?,
                ))
            })
            .context(DatabaseSnafu)?;

        rows.map(|row| {
            let (
                id_str,
                file_path,
                repo_name,
                line_start,
                line_end,
                context_type,
                context_data,
                content,
                last_modified,
                distance,
            ) = row.context(DatabaseSnafu)?;

            let chunk = Self::deserialize_chunk(
                id_str,
                file_path,
                repo_name,
                line_start,
                line_end,
                context_type,
                context_data,
                content,
                last_modified,
            )?;

            let similarity = 1.0 - distance;
            Ok((chunk, similarity))
        })
        .collect()
    }

    fn search_bm25(
        &self,
        _query_terms: &[String],
        _limit: usize,
    ) -> Result<Vec<(Chunk, f32)>, StorageError> {
        use super::repository::storage_error::*;

        Err(UnsupportedOperationSnafu {
            operation: "search_bm25 not yet implemented",
        }
        .build())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use tempfile::NamedTempFile;

    fn create_test_chunk() -> Chunk {
        Chunk::builder()
            .id(ChunkId::new(uuid::Uuid::new_v4()))
            .source(
                crate::knowledge::domain::ChunkSource::builder()
                    .file_path(ForestRelativePath::try_new("test.md").unwrap())
                    .repo_name(crate::knowledge::domain::RepoName::try_new("test-repo").unwrap())
                    .line_range(
                        crate::knowledge::domain::LineRange::builder()
                            .start(crate::knowledge::domain::LineNumber::try_new(1).unwrap())
                            .line_count(crate::knowledge::domain::LineCount::try_new(10).unwrap())
                            .build(),
                    )
                    .build(),
            )
            .content(
                crate::knowledge::domain::ChunkContent::builder()
                    .text("test content")
                    .token_count(crate::knowledge::domain::TokenCount::try_new(10).unwrap())
                    .build(),
            )
            .context(ChunkContext::Markdown(
                crate::knowledge::domain::MarkdownContext::builder()
                    .heading_hierarchy(vec![])
                    .build(),
            ))
            .indexed_at(SystemTime::now())
            .build()
    }

    #[test]
    fn test_save_and_retrieve() {
        // Given A repository and a chunk
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path()).unwrap();
        let chunk = create_test_chunk();

        // When Saving the chunk
        repo.save(&chunk).unwrap();

        // Then It should be retrievable
        let retrieved = repo.find_by_id(&chunk.id).unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id, chunk.id);
        assert_eq!(retrieved.content.text, chunk.content.text);
    }

    #[test]
    fn test_save_batch() {
        // Given A repository and multiple chunks
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path()).unwrap();
        let chunks = vec![create_test_chunk(), create_test_chunk()];

        // When Saving in batch
        repo.save_batch(&chunks).unwrap();

        // Then All chunks should be retrievable
        let all = repo.find_all().unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_find_by_file() {
        // Given A repository with chunks from different files
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path()).unwrap();
        let chunk1 = create_test_chunk();
        let mut chunk2 = create_test_chunk();
        chunk2.source.file_path = ForestRelativePath::try_new("other.md").unwrap();

        repo.save(&chunk1).unwrap();
        repo.save(&chunk2).unwrap();

        // When Finding by file
        let results = repo
            .find_by_file(&ForestRelativePath::try_new("test.md").unwrap())
            .unwrap();

        // Then Only chunks from that file should be returned
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, chunk1.id);
    }

    #[test]
    fn test_delete_by_file() {
        // Given A repository with chunks
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path()).unwrap();
        let chunk = create_test_chunk();
        repo.save(&chunk).unwrap();

        // When Deleting by file
        let count = repo
            .delete_by_file(&ForestRelativePath::try_new("test.md").unwrap())
            .unwrap();

        // Then The chunks should be removed
        assert_eq!(count, 1);
        let all = repo.find_all().unwrap();
        assert_eq!(all.len(), 0);
    }

    #[test]
    fn test_clear() {
        // Given A repository with chunks
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path()).unwrap();
        repo.save(&create_test_chunk()).unwrap();
        repo.save(&create_test_chunk()).unwrap();

        // When Clearing
        let count = repo.clear().unwrap();

        // Then All chunks should be removed
        assert_eq!(count, 2);
        let all = repo.find_all().unwrap();
        assert_eq!(all.len(), 0);
    }

    #[test]
    fn test_metadata() {
        // Given A repository
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path()).unwrap();

        // When Setting metadata
        let metadata = IndexMetadata::builder()
            .mode(IndexMode::Best)
            .last_build(SystemTime::now())
            .chunk_count(10)
            .file_count(5)
            .build();

        repo.set_metadata(&metadata).unwrap();

        // Then It should be retrievable
        let retrieved = repo.get_metadata().unwrap();
        assert_eq!(retrieved.mode, IndexMode::Best);
    }

    #[test]
    fn test_rustdoc_context_roundtrip() {
        // Given A repository and a chunk with RustDoc context
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path()).unwrap();

        let chunk = Chunk::builder()
            .id(ChunkId::new(uuid::Uuid::new_v4()))
            .source(
                crate::knowledge::domain::ChunkSource::builder()
                    .file_path(ForestRelativePath::try_new("src/lib.rs").unwrap())
                    .repo_name(crate::knowledge::domain::RepoName::try_new("twoliter").unwrap())
                    .line_range(
                        crate::knowledge::domain::LineRange::builder()
                            .start(crate::knowledge::domain::LineNumber::try_new(20).unwrap())
                            .line_count(crate::knowledge::domain::LineCount::try_new(11).unwrap())
                            .build(),
                    )
                    .build(),
            )
            .content(
                crate::knowledge::domain::ChunkContent::builder()
                    .text("/// Builds a variant")
                    .token_count(crate::knowledge::domain::TokenCount::try_new(5).unwrap())
                    .build(),
            )
            .context(ChunkContext::RustDoc(
                crate::knowledge::domain::RustDocContext::builder()
                    .item_type(crate::knowledge::domain::RustItemType::Function)
                    .item_name(
                        crate::knowledge::domain::ItemName::try_new("build_variant").unwrap(),
                    )
                    .visibility(crate::knowledge::domain::Visibility::Public)
                    .signature(
                        crate::knowledge::domain::Signature::try_new("pub fn build_variant()")
                            .unwrap(),
                    )
                    .build(),
            ))
            .indexed_at(SystemTime::now())
            .build();

        // When Saving and retrieving the chunk
        repo.save(&chunk).unwrap();
        let retrieved = repo.find_by_id(&chunk.id).unwrap().unwrap();

        // Then The RustDoc context should be preserved
        match retrieved.context {
            ChunkContext::RustDoc(ctx) => {
                assert_eq!(
                    ctx.item_type,
                    crate::knowledge::domain::RustItemType::Function
                );
                assert_eq!(ctx.visibility, crate::knowledge::domain::Visibility::Public);
                assert!(ctx.signature.is_some());
            }
            _ => panic!("Expected RustDoc context"),
        }
    }

    #[test]
    fn test_invalid_context_type_rejected() {
        // Given An invalid context type string
        // When Attempting to deserialize
        let result = SqliteChunkRepository::deserialize_context("invalid_type", "{}");

        // Then It should fail
        assert!(result.is_err());
    }

    #[test]
    fn test_sqlite_vec_extension_loaded() {
        // Given A temporary database
        let temp_file = NamedTempFile::new().unwrap();

        // When Opening the repository
        let repo = SqliteChunkRepository::open(temp_file.path()).unwrap();

        // Then The sqlite-vec extension should be available
        // We can verify this by checking if vec0 module exists
        let result: Result<i32, _> = repo.conn.query_row(
            "SELECT 1 FROM pragma_module_list WHERE name = 'vec0'",
            [],
            |row| row.get(0),
        );

        assert!(result.is_ok());
    }

    #[test]
    fn test_search_semantic_with_embeddings() {
        // Given A repository with chunks that have embeddings
        let temp_file = NamedTempFile::new().unwrap();
        let repo = SqliteChunkRepository::open(temp_file.path()).unwrap();

        let chunk_id = uuid::Uuid::new_v4();
        let embedding: Vec<f32> = (0..384).map(|i| i as f32 / 384.0).collect();
        let embedding_blob = SqliteChunkRepository::serialize_embedding(&embedding);

        let context_data = serde_json::json!({"heading_hierarchy": []}).to_string();

        // Insert a chunk
        repo.conn
            .execute(
                "INSERT INTO chunks (id, file_path, repo_name, line_start, line_end, 
                 context_type, context_data, content, last_modified, index_mode)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    chunk_id.to_string(),
                    "test.md",
                    "test-repo",
                    1,
                    10,
                    "markdown",
                    context_data,
                    "test content",
                    0,
                    "best",
                ],
            )
            .unwrap();

        // Insert embedding into vec_chunks
        repo.conn
            .execute(
                "INSERT INTO vec_chunks (chunk_id, embedding) VALUES (?1, ?2)",
                params![chunk_id.to_string(), embedding_blob],
            )
            .unwrap();

        // When Searching with a similar embedding
        let query_embedding: Vec<f32> = (0..384).map(|i| (i as f32 + 0.1) / 384.0).collect();
        let results = repo.search_semantic(&query_embedding, 10).unwrap();

        // Then The chunk should be found with a similarity score
        assert_eq!(results.len(), 1);
        let (chunk, score) = &results[0];
        assert_eq!(chunk.id.to_string(), chunk_id.to_string());
        assert!(score > &0.0 && score <= &1.0);
    }
}
