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

    /// Reconstruct Chunk from database row fields
    ///
    /// Handles all type conversions and validations needed to build a Chunk
    /// from the raw database values.
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

        macro_rules! try_field {
            ($field:expr, $expr:expr) => {
                $expr
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync + 'static>)
                    .context(InvalidFieldSnafu {
                        field: $field.to_string(),
                    })?
            };
        }

        let uuid = try_field!("chunk_id", uuid::Uuid::parse_str(&id_str));

        let chunk = Chunk::builder()
            .id(ChunkId::new(uuid))
            .source(
                crate::knowledge::domain::ChunkSource::builder()
                    .file_path(try_field!(
                        "file_path",
                        ForestRelativePath::try_new(file_path)
                    ))
                    .repo_name(try_field!(
                        "repo_name",
                        crate::knowledge::domain::RepoName::try_new(repo_name)
                    ))
                    .line_range(
                        crate::knowledge::domain::LineRange::builder()
                            .start(try_field!(
                                "line_start",
                                crate::knowledge::domain::LineNumber::try_new(line_start as usize)
                            ))
                            .line_count(try_field!(
                                "line_count",
                                crate::knowledge::domain::LineCount::try_new(line_end as usize)
                            ))
                            .build(),
                    )
                    .build(),
            )
            .content(
                crate::knowledge::domain::ChunkContent::builder()
                    .text(content.clone())
                    .token_count(try_field!(
                        "token_count",
                        crate::knowledge::domain::TokenCount::try_new(content.len())
                    ))
                    .build(),
            )
            .context(Self::deserialize_context(&context_type, &context_data)?)
            .indexed_at(Self::unix_to_system_time(last_modified))
            .build();

        Ok(chunk)
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
        query_terms: &[String],
        limit: usize,
    ) -> Result<Vec<(Chunk, f32)>, StorageError> {
        use super::repository::storage_error::*;

        if query_terms.is_empty() {
            return Ok(vec![]);
        }

        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, file_path, repo_name, line_start, line_end,
                        context_type, context_data, content, last_modified, bm25_terms
                 FROM chunks
                 WHERE bm25_terms IS NOT NULL",
            )
            .context(DatabaseSnafu)?;

        let rows: Vec<_> = stmt
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
                    row.get::<_, String>(9)?,
                ))
            })
            .context(DatabaseSnafu)?
            .collect::<Result<Vec<_>, _>>()
            .context(DatabaseSnafu)?;

        let total_docs = rows.len() as f32;
        let avg_doc_length = self.calculate_avg_doc_length(&rows)?;
        let idf_scores = self.calculate_idf(query_terms, &rows, total_docs)?;

        let mut scored_chunks: Vec<(Chunk, f32)> = rows
            .into_iter()
            .filter_map(
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
                    bm25_terms,
                )| {
                    let term_freqs: std::collections::HashMap<String, usize> =
                        serde_json::from_str(&bm25_terms).ok()?;

                    let doc_length = term_freqs.values().sum::<usize>() as f32;
                    let score = self.calculate_bm25_score(
                        query_terms,
                        &term_freqs,
                        &idf_scores,
                        doc_length,
                        avg_doc_length,
                    );

                    if score > 0.0 {
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
                        )
                        .ok()?;
                        Some((chunk, score))
                    } else {
                        None
                    }
                },
            )
            .collect();

        scored_chunks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored_chunks.truncate(limit);

        Ok(scored_chunks)
    }
}

impl SqliteChunkRepository {
    /// Calculate average document length across all chunks
    ///
    /// Used in BM25 scoring to normalize for document length.
    /// Document length is the sum of all term frequencies.
    fn calculate_avg_doc_length(
        &self,
        rows: &[(
            String,
            String,
            String,
            i64,
            i64,
            String,
            String,
            String,
            i64,
            String,
        )],
    ) -> Result<f32, StorageError> {
        let total_length: usize = rows
            .iter()
            .filter_map(|(_, _, _, _, _, _, _, _, _, bm25_terms)| {
                let term_freqs: std::collections::HashMap<String, usize> =
                    serde_json::from_str(bm25_terms).ok()?;
                Some(term_freqs.values().sum::<usize>())
            })
            .sum();

        Ok(total_length as f32 / rows.len().max(1) as f32)
    }

    /// Calculate IDF (Inverse Document Frequency) for query terms
    ///
    /// IDF measures how rare a term is across the corpus. Rare terms get higher scores.
    /// Formula: ln((N - df + 0.5) / (df + 0.5) + 1) where N is total docs, df is doc frequency.
    fn calculate_idf(
        &self,
        query_terms: &[String],
        rows: &[(
            String,
            String,
            String,
            i64,
            i64,
            String,
            String,
            String,
            i64,
            String,
        )],
        total_docs: f32,
    ) -> Result<std::collections::HashMap<String, f32>, StorageError> {
        let mut doc_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        for (_, _, _, _, _, _, _, _, _, bm25_terms) in rows {
            if let Ok(term_freqs) =
                serde_json::from_str::<std::collections::HashMap<String, usize>>(bm25_terms)
            {
                for term in query_terms {
                    if term_freqs.contains_key(term) {
                        *doc_counts.entry(term.clone()).or_insert(0) += 1;
                    }
                }
            }
        }

        let mut idf_scores = std::collections::HashMap::new();
        for term in query_terms {
            let doc_freq = *doc_counts.get(term).unwrap_or(&0) as f32;
            let idf = ((total_docs - doc_freq + 0.5) / (doc_freq + 0.5) + 1.0).ln();
            idf_scores.insert(term.clone(), idf);
        }

        Ok(idf_scores)
    }

    /// Calculate BM25 score for a document given query terms
    ///
    /// BM25 combines term frequency (how often term appears in doc) with IDF
    /// (how rare the term is) and normalizes by document length.
    ///
    /// Uses standard parameters: k1=1.2 (term frequency saturation), b=0.75 (length normalization).
    fn calculate_bm25_score(
        &self,
        query_terms: &[String],
        term_freqs: &std::collections::HashMap<String, usize>,
        idf_scores: &std::collections::HashMap<String, f32>,
        doc_length: f32,
        avg_doc_length: f32,
    ) -> f32 {
        const K1: f32 = 1.2;
        const B: f32 = 0.75;

        let mut score = 0.0;
        for term in query_terms {
            if let Some(&tf) = term_freqs.get(term) {
                let idf = idf_scores.get(term).unwrap_or(&0.0);
                let tf = tf as f32;
                let numerator = tf * (K1 + 1.0);
                let denominator = tf + K1 * (1.0 - B + B * (doc_length / avg_doc_length));
                score += idf * (numerator / denominator);
            }
        }
        score
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

    #[test]
    fn test_save_with_embedding_stores_in_both_tables() {
        // Given A repository and a chunk with an embedding
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path()).unwrap();

        let embedding: Vec<f32> = (0..384).map(|i| i as f32 / 384.0).collect();
        let chunk = Chunk::builder()
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
                crate::knowledge::domain::ChunkSource::builder()
                    .file_path(ForestRelativePath::try_new("test1.md").unwrap())
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
                    .text("test content 1")
                    .token_count(crate::knowledge::domain::TokenCount::try_new(10).unwrap())
                    .build(),
            )
            .context(ChunkContext::Markdown(
                crate::knowledge::domain::MarkdownContext::builder()
                    .heading_hierarchy(vec![])
                    .build(),
            ))
            .indexed_at(SystemTime::now())
            .embedding(embedding1)
            .build();

        let chunk2 = Chunk::builder()
            .id(ChunkId::new(uuid::Uuid::new_v4()))
            .source(
                crate::knowledge::domain::ChunkSource::builder()
                    .file_path(ForestRelativePath::try_new("test2.md").unwrap())
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
                    .text("test content 2")
                    .token_count(crate::knowledge::domain::TokenCount::try_new(10).unwrap())
                    .build(),
            )
            .context(ChunkContext::Markdown(
                crate::knowledge::domain::MarkdownContext::builder()
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
