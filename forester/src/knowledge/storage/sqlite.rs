//! SQLite implementation of ChunkRepository

use rusqlite::{Connection, OptionalExtension, params};
use snafu::ResultExt;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use super::repository::{ChunkRepository, IndexMetadata, StorageError};
use super::schema;
use crate::knowledge::domain::{
    Chunk, ChunkContent, ChunkContext, ChunkId, ChunkSource, ForestRelativePath, IndexMode, LineCount, LineNumber, LineRange, RepoName, TokenCount,
};

#[cfg(test)]
use crate::knowledge::domain::{ItemName, MarkdownContext, RustDocContext, RustItemType, Signature, Visibility};

/// SQLite-backed chunk repository
pub struct SqliteChunkRepository {
    conn: Connection,
}

/// Macro to parse a Chunk directly from a rusqlite::Row
///
/// Expects columns in order: id, file_path, repo_name, line_start, line_end,
/// context_type, context_data, content, last_modified
///
/// Returns Result<Chunk, rusqlite::Error> for use in query_map closures
macro_rules! chunk_from_row {
    ($row:expr) => {{
        (|| -> Result<Chunk, rusqlite::Error> {
            use super::repository::storage_error::*;
            use snafu::ResultExt;

            let id_str: String = $row.get(0)?;
            let file_path: String = $row.get(1)?;
            let repo_name: String = $row.get(2)?;
            let line_start: i64 = $row.get(3)?;
            let line_end: i64 = $row.get(4)?;
            let context_type: String = $row.get(5)?;
            let context_data: String = $row.get(6)?;
            let content: String = $row.get(7)?;
            let last_modified: i64 = $row.get(8)?;

            let uuid = uuid::Uuid::parse_str(&id_str)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync + 'static>)
                .context(InvalidFieldSnafu {
                    field: "chunk_id".to_string(),
                })
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

            let context = SqliteChunkRepository::deserialize_context(&context_type, &context_data)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

            Ok(Chunk::builder()
                .id(ChunkId::new(uuid))
                .source(
                    ChunkSource::builder()
                        .file_path(
                            ForestRelativePath::try_new(file_path)
                                .map_err(|e| {
                                    Box::new(e) as Box<dyn std::error::Error + Send + Sync + 'static>
                                })
                                .context(InvalidFieldSnafu {
                                    field: "file_path".to_string(),
                                })
                                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                        )
                        .repo_name(
                            RepoName::try_new(repo_name)
                                .map_err(|e| {
                                    Box::new(e) as Box<dyn std::error::Error + Send + Sync + 'static>
                                })
                                .context(InvalidFieldSnafu {
                                    field: "repo_name".to_string(),
                                })
                                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                        )
                        .line_range(
                            LineRange::builder()
                                .start(
                                    LineNumber::try_new(line_start as usize)
                                        .map_err(|e| {
                                            Box::new(e)
                                                as Box<dyn std::error::Error + Send + Sync + 'static>
                                        })
                                        .context(InvalidFieldSnafu {
                                            field: "line_start".to_string(),
                                        })
                                        .map_err(|e| {
                                            rusqlite::Error::ToSqlConversionFailure(Box::new(e))
                                        })?,
                                )
                                .line_count(
                                    LineCount::try_new(line_end as usize)
                                        .map_err(|e| {
                                            Box::new(e)
                                                as Box<dyn std::error::Error + Send + Sync + 'static>
                                        })
                                        .context(InvalidFieldSnafu {
                                            field: "line_count".to_string(),
                                        })
                                        .map_err(|e| {
                                            rusqlite::Error::ToSqlConversionFailure(Box::new(e))
                                        })?,
                                )
                                .build(),
                        )
                        .build(),
                )
                .content(
                    ChunkContent::builder()
                        .text(content.clone())
                        .token_count(
                            TokenCount::try_new(content.len())
                                .map_err(|e| {
                                    Box::new(e) as Box<dyn std::error::Error + Send + Sync + 'static>
                                })
                                .context(InvalidFieldSnafu {
                                    field: "token_count".to_string(),
                                })
                                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                        )
                        .build(),
                )
                .context(context)
                .indexed_at(SqliteChunkRepository::unix_to_system_time(last_modified))
                .build())
        })()
    }};
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

        // Register custom LN function (natural logarithm)
        conn.create_scalar_function(
            "LN",
            1,
            rusqlite::functions::FunctionFlags::SQLITE_UTF8
                | rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC,
            |ctx| {
                let value = ctx.get::<f64>(0)?;
                Ok(value.ln())
            },
        )
        .context(DatabaseSnafu)?;

        schema::create_tables(&conn).map_err(|e| {
            InvalidDataSnafu {
                message: e.to_string(),
            }
            .build()
        })?;

        Ok(Self { conn })
    }

    /// Convert f32 embedding vector to bytes for sqlite-vec storage
    ///
    /// sqlite-vec expects embeddings as little-endian f32 bytes.
    fn serialize_embedding(embedding: &[f32]) -> Vec<u8> {
        embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
    }

    /// Convert ChunkContext enum to database-storable format
    ///
    /// Returns (context_type, context_data_json) tuple for storage in chunks table.
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

    /// Reconstruct ChunkContext from database fields
    ///
    /// Parses context_data JSON based on context_type discriminator.
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

    /// Convert SystemTime to Unix timestamp for database storage
    fn system_time_to_unix(time: SystemTime) -> i64 {
        time.duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
    }

    /// Convert Unix timestamp from database to SystemTime
    fn unix_to_system_time(unix: i64) -> SystemTime {
        UNIX_EPOCH + std::time::Duration::from_secs(unix as u64)
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

        if let Some(embedding) = &chunk.embedding {
            let embedding_blob = Self::serialize_embedding(embedding);
            self.conn
                .execute(
                    "INSERT OR REPLACE INTO vec_chunks (chunk_id, embedding) VALUES (?1, ?2)",
                    params![chunk.id.to_string(), embedding_blob],
                )
                .context(DatabaseSnafu)?;
        }

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

            if let Some(embedding) = &chunk.embedding {
                let embedding_blob = Self::serialize_embedding(embedding);
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

        stmt.query_row(params![id.to_string()], |row| chunk_from_row!(row))
            .optional()
            .context(DatabaseSnafu)
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

        stmt
            .query_map(params![path.to_string()], |row| chunk_from_row!(row))
            .context(DatabaseSnafu)?
            .collect::<Result<Vec<_>, _>>()
            .context(DatabaseSnafu)
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

        stmt
            .query_map([], |row| chunk_from_row!(row))
            .context(DatabaseSnafu)?
            .collect::<Result<Vec<_>, _>>()
            .context(DatabaseSnafu)
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

        stmt
            .query_map(params![embedding_blob, limit], |row| {
                let chunk = chunk_from_row!(row)?;
                let distance: f32 = row.get(9)?;
                let similarity = 1.0 - distance;
                Ok((chunk, similarity))
            })
            .context(DatabaseSnafu)?
            .collect::<Result<Vec<_>, _>>()
            .context(DatabaseSnafu)
    }

    fn search_bm25(
        &self,
        query_terms: &[String],
        limit: usize,
    ) -> Result<Vec<(Chunk, f32)>, StorageError> {
        use super::repository::storage_error::*;

        if query_terms.is_empty() {
            return Ok(vec![]);
        }

        // Build SQL query with BM25 calculation
        // Uses CTEs to compute corpus statistics and per-document scores
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
                    c.line_end,
                    c.context_type,
                    c.context_data,
                    c.content,
                    c.last_modified,
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

        let mut stmt = self.conn.prepare(&query).context(DatabaseSnafu)?;

        // Bind query terms twice: once for IN clause, once for limit
        let mut params: Vec<&dyn rusqlite::ToSql> = query_terms
            .iter()
            .map(|t| t as &dyn rusqlite::ToSql)
            .collect();
        params.push(&limit);

        let results = stmt
            .query_map(params.as_slice(), |row| {
                let chunk = chunk_from_row!(row)?;
                let score: f32 = row.get(9)?;
                Ok((chunk, score))
            })
            .context(DatabaseSnafu)?
            .collect::<Result<Vec<_>, _>>()
            .context(DatabaseSnafu)?;

        Ok(results)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::knowledge::domain::{ItemName, MarkdownContext, RustDocContext, RustItemType, Signature, Visibility};
    use tempfile::NamedTempFile;

    fn create_test_chunk() -> Chunk {
        Chunk::builder()
            .id(ChunkId::new(uuid::Uuid::new_v4()))
            .source(
                ChunkSource::builder()
                    .file_path(ForestRelativePath::try_new("test.md").unwrap())
                    .repo_name(RepoName::try_new("test-repo").unwrap())
                    .line_range(
                        LineRange::builder()
                            .start(LineNumber::try_new(1).unwrap())
                            .line_count(LineCount::try_new(10).unwrap())
                            .build(),
                    )
                    .build(),
            )
            .content(
                ChunkContent::builder()
                    .text("test content")
                    .token_count(TokenCount::try_new(10).unwrap())
                    .build(),
            )
            .context(ChunkContext::Markdown(
                MarkdownContext::builder()
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
                ChunkSource::builder()
                    .file_path(ForestRelativePath::try_new("src/lib.rs").unwrap())
                    .repo_name(RepoName::try_new("twoliter").unwrap())
                    .line_range(
                        LineRange::builder()
                            .start(LineNumber::try_new(20).unwrap())
                            .line_count(LineCount::try_new(11).unwrap())
                            .build(),
                    )
                    .build(),
            )
            .content(
                ChunkContent::builder()
                    .text("/// Builds a variant")
                    .token_count(TokenCount::try_new(5).unwrap())
                    .build(),
            )
            .context(ChunkContext::RustDoc(
                RustDocContext::builder()
                    .item_type(RustItemType::Function)
                    .item_name(
                        ItemName::try_new("build_variant").unwrap(),
                    )
                    .visibility(Visibility::Public)
                    .signature(
                        Signature::try_new("pub fn build_variant()")
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
                    RustItemType::Function
                );
                assert_eq!(ctx.visibility, Visibility::Public);
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

    #[test]
    fn test_save_with_embedding_stores_in_both_tables() {
        // Given A repository and a chunk with an embedding
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path()).unwrap();

        let embedding: Vec<f32> = (0..384).map(|i| i as f32 / 384.0).collect();
        let chunk = Chunk::builder()
            .id(ChunkId::new(uuid::Uuid::new_v4()))
            .source(
                ChunkSource::builder()
                    .file_path(ForestRelativePath::try_new("test.md").unwrap())
                    .repo_name(RepoName::try_new("test-repo").unwrap())
                    .line_range(
                        LineRange::builder()
                            .start(LineNumber::try_new(1).unwrap())
                            .line_count(LineCount::try_new(10).unwrap())
                            .build(),
                    )
                    .build(),
            )
            .content(
                ChunkContent::builder()
                    .text("test content")
                    .token_count(TokenCount::try_new(10).unwrap())
                    .build(),
            )
            .context(ChunkContext::Markdown(
                MarkdownContext::builder()
                    .heading_hierarchy(vec![])
                    .build(),
            ))
            .indexed_at(SystemTime::now())
            .embedding(embedding)
            .build();

        // When Saving the chunk
        repo.save(&chunk).unwrap();

        // Then It should be in both chunks and vec_chunks tables
        let chunk_exists: bool = repo
            .conn
            .query_row(
                "SELECT 1 FROM chunks WHERE id = ?1",
                params![chunk.id.to_string()],
                |_| Ok(true),
            )
            .unwrap();
        assert!(chunk_exists);

        let vec_chunk_exists: bool = repo
            .conn
            .query_row(
                "SELECT 1 FROM vec_chunks WHERE chunk_id = ?1",
                params![chunk.id.to_string()],
                |_| Ok(true),
            )
            .unwrap();
        assert!(vec_chunk_exists);
    }

    #[test]
    fn test_save_without_embedding_skips_vec_chunks() {
        // Given A repository and a chunk without an embedding
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path()).unwrap();

        let chunk = create_test_chunk();

        // When Saving the chunk
        repo.save(&chunk).unwrap();

        // Then It should be in chunks but not vec_chunks
        let chunk_exists: bool = repo
            .conn
            .query_row(
                "SELECT 1 FROM chunks WHERE id = ?1",
                params![chunk.id.to_string()],
                |_| Ok(true),
            )
            .unwrap();
        assert!(chunk_exists);

        let vec_chunk_result: Result<bool, _> = repo.conn.query_row(
            "SELECT 1 FROM vec_chunks WHERE chunk_id = ?1",
            params![chunk.id.to_string()],
            |_| Ok(true),
        );
        assert!(vec_chunk_result.is_err());
    }

    #[test]
    fn test_save_batch_with_embeddings() {
        // Given A repository and chunks with embeddings
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path()).unwrap();

        let embedding1: Vec<f32> = (0..384).map(|i| i as f32 / 384.0).collect();
        let embedding2: Vec<f32> = (0..384).map(|i| (i as f32 + 0.5) / 384.0).collect();

        let chunk1 = Chunk::builder()
            .id(ChunkId::new(uuid::Uuid::new_v4()))
            .source(
                ChunkSource::builder()
                    .file_path(ForestRelativePath::try_new("test1.md").unwrap())
                    .repo_name(RepoName::try_new("test-repo").unwrap())
                    .line_range(
                        LineRange::builder()
                            .start(LineNumber::try_new(1).unwrap())
                            .line_count(LineCount::try_new(10).unwrap())
                            .build(),
                    )
                    .build(),
            )
            .content(
                ChunkContent::builder()
                    .text("test content 1")
                    .token_count(TokenCount::try_new(10).unwrap())
                    .build(),
            )
            .context(ChunkContext::Markdown(
                MarkdownContext::builder()
                    .heading_hierarchy(vec![])
                    .build(),
            ))
            .indexed_at(SystemTime::now())
            .embedding(embedding1)
            .build();

        let chunk2 = Chunk::builder()
            .id(ChunkId::new(uuid::Uuid::new_v4()))
            .source(
                ChunkSource::builder()
                    .file_path(ForestRelativePath::try_new("test2.md").unwrap())
                    .repo_name(RepoName::try_new("test-repo").unwrap())
                    .line_range(
                        LineRange::builder()
                            .start(LineNumber::try_new(1).unwrap())
                            .line_count(LineCount::try_new(10).unwrap())
                            .build(),
                    )
                    .build(),
            )
            .content(
                ChunkContent::builder()
                    .text("test content 2")
                    .token_count(TokenCount::try_new(10).unwrap())
                    .build(),
            )
            .context(ChunkContext::Markdown(
                MarkdownContext::builder()
                    .heading_hierarchy(vec![])
                    .build(),
            ))
            .indexed_at(SystemTime::now())
            .embedding(embedding2)
            .build();

        // When Saving in batch
        repo.save_batch(&[chunk1.clone(), chunk2.clone()]).unwrap();

        // Then Both should be in vec_chunks
        let count: i64 = repo
            .conn
            .query_row("SELECT COUNT(*) FROM vec_chunks", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_search_bm25() {
        // Given A repository with chunks that have BM25 term frequencies
        let temp_file = NamedTempFile::new().unwrap();
        let repo = SqliteChunkRepository::open(temp_file.path()).unwrap();

        let chunk1_id = uuid::Uuid::new_v4();
        let chunk2_id = uuid::Uuid::new_v4();

        let bm25_terms1 = serde_json::json!({"rust": 5, "documentation": 3, "code": 2}).to_string();
        let bm25_terms2 = serde_json::json!({"rust": 2, "testing": 4, "code": 1}).to_string();
        let context_data = serde_json::json!({"heading_hierarchy": []}).to_string();

        repo.conn
            .execute(
                "INSERT INTO chunks (id, file_path, repo_name, line_start, line_end,
                 context_type, context_data, content, last_modified, index_mode, bm25_terms)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    chunk1_id.to_string(),
                    "test1.md",
                    "test-repo",
                    1,
                    10,
                    "markdown",
                    context_data,
                    "rust documentation code",
                    0,
                    "fast",
                    bm25_terms1,
                ],
            )
            .unwrap();

        repo.conn
            .execute(
                "INSERT INTO chunks (id, file_path, repo_name, line_start, line_end,
                 context_type, context_data, content, last_modified, index_mode, bm25_terms)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    chunk2_id.to_string(),
                    "test2.md",
                    "test-repo",
                    1,
                    10,
                    "markdown",
                    context_data,
                    "rust testing code",
                    0,
                    "fast",
                    bm25_terms2,
                ],
            )
            .unwrap();

        // When Searching for "rust documentation"
        let query_terms = vec!["rust".to_string(), "documentation".to_string()];
        let results = repo.search_bm25(&query_terms, 10).unwrap();

        // Then Chunk1 should rank higher (has "documentation")
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0.id.to_string(), chunk1_id.to_string());
        assert!(results[0].1 > results[1].1);
    }
}
