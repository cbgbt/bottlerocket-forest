//! SQLite implementation of ChunkRepository
//!
//! - `serialization`: Converting between domain types and database formats
//! - `queries`: CRUD operations for chunk storage
//! - `search`: Semantic and BM25 search implementations

mod queries;
mod search;
mod serialization;

use rusqlite::Connection;
use snafu::ResultExt;
use std::path::Path;

use super::repository::{ChunkRepository, IndexMetadata, StorageError, storage_error::*};
use super::schema;
use crate::knowledge::domain::{Chunk, ChunkId, ForestRelativePath};

/// SQLite-backed chunk repository
pub struct SqliteChunkRepository {
    conn: Connection,
}

impl SqliteChunkRepository {
    /// Open or create a database at the given path
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        // SAFETY: This call satisfies the safety requirements for sqlite3_auto_extension:
        // 1. We are not calling this from within an auto-extension handler (would cause recursion)
        // 2. We will not close any database connection from within the auto-extension
        // 3. We will not manipulate the auto-extension list from within an auto-extension
        // 4. sqlite3_vec_init is a valid C function pointer with the correct signature
        //    provided by the sqlite-vec crate's extern "C" block
        // 5. The transmute is valid because:
        //    - Both types are function pointers with the same size
        //    - sqlite3_vec_init has the correct signature expected by sqlite3_auto_extension
        //    - The function pointer is statically linked and will remain valid
        #[allow(clippy::missing_transmute_annotations)]
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }

        let conn = Connection::open(path.as_ref()).context(DatabaseSnafu)?;

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
}

impl ChunkRepository for SqliteChunkRepository {
    fn save(&mut self, chunk: &Chunk) -> Result<(), StorageError> {
        queries::save(&mut self.conn, chunk)
    }

    fn save_batch(&mut self, chunks: &[Chunk]) -> Result<(), StorageError> {
        queries::save_batch(&mut self.conn, chunks)
    }

    fn find_by_id(&self, id: &ChunkId) -> Result<Option<Chunk>, StorageError> {
        queries::find_by_id(&self.conn, id)
    }

    fn find_by_file(&self, path: &ForestRelativePath) -> Result<Vec<Chunk>, StorageError> {
        queries::find_by_file(&self.conn, path)
    }

    fn find_all(&self) -> Result<Vec<Chunk>, StorageError> {
        queries::find_all(&self.conn)
    }

    fn delete_by_file(&mut self, path: &ForestRelativePath) -> Result<usize, StorageError> {
        queries::delete_by_file(&mut self.conn, path)
    }

    fn clear(&mut self) -> Result<usize, StorageError> {
        queries::clear(&mut self.conn)
    }

    fn get_metadata(&self) -> Result<IndexMetadata, StorageError> {
        queries::get_metadata(&self.conn)
    }

    fn set_metadata(&mut self, metadata: &IndexMetadata) -> Result<(), StorageError> {
        queries::set_metadata(&mut self.conn, metadata)
    }

    fn search_semantic(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<(Chunk, f32)>, StorageError> {
        search::search_semantic(&self.conn, query_embedding, limit)
    }

    fn search_bm25(
        &self,
        query_terms: &[String],
        limit: usize,
    ) -> Result<Vec<(Chunk, f32)>, StorageError> {
        search::search_bm25(&self.conn, query_terms, limit)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::knowledge::domain::{
        ChunkContent, ChunkContext, ChunkSource, HeadingText, IndexData, IndexMode, ItemName,
        LineCount, LineNumber, LineRange, MarkdownContext, RepoName, RustDocContext, RustItemType,
        Signature, TokenCount, Visibility,
    };
    use std::time::SystemTime;
    use tempfile::NamedTempFile;
    use test_case::test_case;

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
                MarkdownContext::builder().heading_hierarchy(vec![]).build(),
            ))
            .indexed_at(SystemTime::now())
            .index_data(IndexData::Fast {
                bm25_terms: std::collections::BTreeMap::new(),
            })
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
            .mode(crate::knowledge::domain::IndexMode::Best)
            .last_build(SystemTime::now())
            .chunk_count(10)
            .file_count(5)
            .build();

        repo.set_metadata(&metadata).unwrap();

        // Then It should be retrievable
        let retrieved = repo.get_metadata().unwrap();
        assert_eq!(retrieved.mode, crate::knowledge::domain::IndexMode::Best);
    }

    #[test_case(
        ChunkContext::Markdown(MarkdownContext::builder().heading_hierarchy(vec![]).build())
        ; "markdown context with empty hierarchy"
    )]
    #[test_case(
        ChunkContext::Markdown(
            MarkdownContext::builder()
                .heading_hierarchy(vec![
                    HeadingText::try_new("Architecture").unwrap(),
                    HeadingText::try_new("Boot Process").unwrap(),
                ])
                .build()
        )
        ; "markdown context with hierarchy"
    )]
    #[test_case(
        ChunkContext::RustDoc(
            RustDocContext::builder()
                .item_type(RustItemType::Function)
                .item_name(ItemName::try_new("build_variant").unwrap())
                .visibility(Visibility::Public)
                .signature(Signature::try_new("pub fn build_variant()").unwrap())
                .build()
        )
        ; "rustdoc context with signature"
    )]
    #[test_case(
        ChunkContext::RustDoc(
            RustDocContext::builder()
                .item_type(RustItemType::Struct)
                .item_name(ItemName::try_new("Config").unwrap())
                .visibility(Visibility::Private)
                .build()
        )
        ; "rustdoc context without signature"
    )]
    fn test_context_roundtrip(context: ChunkContext) {
        // Given A repository and a chunk with specific context
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path()).unwrap();

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
            .context(context.clone())
            .indexed_at(SystemTime::now())
            .index_data(IndexData::Fast {
                bm25_terms: std::collections::BTreeMap::new(),
            })
            .build();

        // When Saving and retrieving the chunk
        repo.save(&chunk).unwrap();
        let retrieved = repo.find_by_id(&chunk.id).unwrap().unwrap();

        // Then The context should be preserved with correct type
        assert_eq!(retrieved.context, context);
    }

    #[test]
    fn test_invalid_context_type_rejected() {
        // Given An invalid context type string
        // When Attempting to deserialize
        let result = serialization::deserialize_context("invalid_type", "{}");

        // Then It should fail
        assert!(result.is_err());
    }

    #[test]
    fn test_search_semantic_with_embeddings() {
        // Given A repository with chunks that have embeddings
        let temp_file = NamedTempFile::new().unwrap();
        let repo = SqliteChunkRepository::open(temp_file.path()).unwrap();

        let chunk_id = uuid::Uuid::new_v4();
        let embedding: Vec<f32> = (0..384).map(|i| i as f32 / 384.0).collect();
        let embedding_blob = serialization::serialize_embedding(&embedding);

        let context_data = serde_json::json!({"heading_hierarchy": []}).to_string();

        repo.conn
            .execute(
                "INSERT INTO chunks (id, file_path, repo_name, line_start, line_count, 
                 context_type, context_data, content, last_modified, index_mode)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                rusqlite::params![
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

        repo.conn
            .execute(
                "INSERT INTO vec_chunks (chunk_id, embedding) VALUES (?1, ?2)",
                rusqlite::params![chunk_id.to_string(), embedding_blob],
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
                MarkdownContext::builder().heading_hierarchy(vec![]).build(),
            ))
            .indexed_at(SystemTime::now())
            .index_data(IndexData::Best { embedding })
            .build();

        // When Saving the chunk
        repo.save(&chunk).unwrap();

        // Then It should be in both chunks and vec_chunks tables
        let chunk_exists: bool = repo
            .conn
            .query_row(
                "SELECT 1 FROM chunks WHERE id = ?1",
                rusqlite::params![chunk.id.to_string()],
                |_| Ok(true),
            )
            .unwrap();
        assert!(chunk_exists);

        let vec_chunk_exists: bool = repo
            .conn
            .query_row(
                "SELECT 1 FROM vec_chunks WHERE chunk_id = ?1",
                rusqlite::params![chunk.id.to_string()],
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
                rusqlite::params![chunk.id.to_string()],
                |_| Ok(true),
            )
            .unwrap();
        assert!(chunk_exists);

        let vec_chunk_result: Result<bool, _> = repo.conn.query_row(
            "SELECT 1 FROM vec_chunks WHERE chunk_id = ?1",
            rusqlite::params![chunk.id.to_string()],
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
                MarkdownContext::builder().heading_hierarchy(vec![]).build(),
            ))
            .indexed_at(SystemTime::now())
            .index_data(IndexData::Best {
                embedding: embedding1,
            })
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
                MarkdownContext::builder().heading_hierarchy(vec![]).build(),
            ))
            .indexed_at(SystemTime::now())
            .index_data(IndexData::Best {
                embedding: embedding2,
            })
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
                "INSERT INTO chunks (id, file_path, repo_name, line_start, line_count,
                 context_type, context_data, content, last_modified, index_mode, bm25_terms)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                rusqlite::params![
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
                "INSERT INTO chunks (id, file_path, repo_name, line_start, line_count,
                 context_type, context_data, content, last_modified, index_mode, bm25_terms)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                rusqlite::params![
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

    #[test]
    fn test_fast_mode_roundtrip() {
        // Given A repository and a Fast mode chunk with BM25 terms
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path()).unwrap();

        let mut bm25_terms = std::collections::BTreeMap::new();
        bm25_terms.insert("rust".to_string(), 5);
        bm25_terms.insert("programming".to_string(), 3);

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
                    .text("rust programming language")
                    .token_count(TokenCount::try_new(25).unwrap())
                    .build(),
            )
            .context(ChunkContext::Markdown(
                MarkdownContext::builder().heading_hierarchy(vec![]).build(),
            ))
            .indexed_at(SystemTime::now())
            .index_data(IndexData::Fast {
                bm25_terms: bm25_terms.clone(),
            })
            .build();

        // When Saving and retrieving
        repo.save(&chunk).unwrap();
        let retrieved = repo.find_by_id(&chunk.id).unwrap().unwrap();

        // Then BM25 terms are preserved and mode is correct
        assert_eq!(retrieved.index_data.mode(), IndexMode::Fast);
        match retrieved.index_data {
            IndexData::Fast {
                bm25_terms: retrieved_terms,
            } => {
                assert_eq!(retrieved_terms, bm25_terms);
            }
            _ => panic!("Expected Fast mode"),
        }
    }

    #[test]
    fn test_best_mode_roundtrip() {
        // Given A repository and a Best mode chunk with embedding
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
                    .text("semantic search content")
                    .token_count(TokenCount::try_new(23).unwrap())
                    .build(),
            )
            .context(ChunkContext::Markdown(
                MarkdownContext::builder().heading_hierarchy(vec![]).build(),
            ))
            .indexed_at(SystemTime::now())
            .index_data(IndexData::Best {
                embedding: embedding.clone(),
            })
            .build();

        // When Saving and retrieving
        repo.save(&chunk).unwrap();
        let retrieved = repo.find_by_id(&chunk.id).unwrap().unwrap();

        // Then Mode is correct (embedding not retrieved in regular queries)
        assert_eq!(retrieved.index_data.mode(), IndexMode::Best);
        match retrieved.index_data {
            IndexData::Best { embedding: _ } => {}
            _ => panic!("Expected Best mode"),
        }
    }

    #[test]
    fn test_fast_mode_empty_terms() {
        // Given A Fast mode chunk with empty BM25 terms
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path()).unwrap();

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
                    .text("the a an")
                    .token_count(TokenCount::try_new(8).unwrap())
                    .build(),
            )
            .context(ChunkContext::Markdown(
                MarkdownContext::builder().heading_hierarchy(vec![]).build(),
            ))
            .indexed_at(SystemTime::now())
            .index_data(IndexData::Fast {
                bm25_terms: std::collections::BTreeMap::new(),
            })
            .build();

        // When Saving and retrieving
        repo.save(&chunk).unwrap();
        let retrieved = repo.find_by_id(&chunk.id).unwrap().unwrap();

        // Then Empty terms are preserved
        match retrieved.index_data {
            IndexData::Fast { bm25_terms } => {
                assert!(bm25_terms.is_empty());
            }
            _ => panic!("Expected Fast mode"),
        }
    }

    #[test]
    fn test_best_mode_zero_embedding() {
        // Given A Best mode chunk with zero-length embedding
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path()).unwrap();

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
                    .text("test")
                    .token_count(TokenCount::try_new(4).unwrap())
                    .build(),
            )
            .context(ChunkContext::Markdown(
                MarkdownContext::builder().heading_hierarchy(vec![]).build(),
            ))
            .indexed_at(SystemTime::now())
            .index_data(IndexData::Best { embedding: vec![] })
            .build();

        // When Saving (should fail because sqlite-vec doesn't support zero-length vectors)
        let result = repo.save(&chunk);

        // Then It should fail
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_fast_mode_missing_terms() {
        // Given A database with Fast mode chunk but NULL bm25_terms
        let temp_file = NamedTempFile::new().unwrap();
        let repo = SqliteChunkRepository::open(temp_file.path()).unwrap();

        let chunk_id = uuid::Uuid::new_v4();
        let context_data = serde_json::json!({"heading_hierarchy": []}).to_string();

        repo.conn
            .execute(
                "INSERT INTO chunks (id, file_path, repo_name, line_start, line_count,
                 context_type, context_data, content, last_modified, index_mode, bm25_terms)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                rusqlite::params![
                    chunk_id.to_string(),
                    "test.md",
                    "test-repo",
                    1,
                    10,
                    "markdown",
                    context_data,
                    "test",
                    0,
                    "fast",
                    None::<String>,
                ],
            )
            .unwrap();

        // When Attempting to retrieve
        let result = repo.find_by_id(&ChunkId::new(chunk_id));

        // Then It should fail with InvalidData error
        assert!(result.is_err());
    }
}
