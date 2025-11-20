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

use super::repository::{ChunkRepository, StorageError, storage_error::*};
use super::schema;
use crate::knowledge::domain::{
    ChunkId, EmbeddingModelConfig, ForestRelativePath, IndexMetadata, IndexedChunk,
};

/// SQLite-backed chunk repository
#[derive(Debug)]
pub struct SqliteChunkRepository {
    conn: Connection,
}

impl SqliteChunkRepository {
    /// Open or create a database at the given path
    pub fn open(
        path: impl AsRef<Path>,
        config: &EmbeddingModelConfig,
    ) -> Result<Self, StorageError> {
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

        schema::create_tables(&conn, config).map_err(|e| {
            InvalidDataSnafu {
                message: e.to_string(),
            }
            .build()
        })?;

        Ok(Self { conn })
    }

    /// Open an existing database and validate the model configuration matches
    pub fn open_with_config(
        path: impl AsRef<Path>,
        expected_config: &EmbeddingModelConfig,
    ) -> Result<Self, StorageError> {
        let repo = Self::open(path, expected_config)?;
        let metadata = repo.get_metadata()?;

        snafu::ensure!(
            metadata.model_config == *expected_config,
            ConfigMismatchSnafu {
                expected: expected_config.clone(),
                actual: metadata.model_config,
            }
        );

        Ok(repo)
    }
}

impl ChunkRepository for SqliteChunkRepository {
    fn save(&mut self, chunk: &IndexedChunk) -> Result<(), StorageError> {
        queries::save(&mut self.conn, chunk)
    }

    fn save_batch(&mut self, chunks: &[IndexedChunk]) -> Result<(), StorageError> {
        queries::save_batch(&mut self.conn, chunks)
    }

    fn find_by_id(&self, id: &ChunkId) -> Result<Option<IndexedChunk>, StorageError> {
        queries::find_by_id(&self.conn, id)
    }

    fn find_by_file(&self, path: &ForestRelativePath) -> Result<Vec<IndexedChunk>, StorageError> {
        queries::find_by_file(&self.conn, path)
    }

    fn find_all(&self) -> Result<Vec<IndexedChunk>, StorageError> {
        queries::find_all(&self.conn)
    }

    fn get_indexed_files(
        &self,
    ) -> Result<
        std::collections::HashMap<ForestRelativePath, crate::knowledge::domain::Timestamp>,
        StorageError,
    > {
        queries::get_indexed_files(&self.conn)
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
    ) -> Result<Vec<(IndexedChunk, f32)>, StorageError> {
        search::search_semantic(&self.conn, query_embedding, limit)
    }

    fn search_bm25(
        &self,
        query_terms: &[String],
        limit: usize,
    ) -> Result<Vec<(IndexedChunk, f32)>, StorageError> {
        search::search_bm25(&self.conn, query_terms, limit)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::knowledge::constants::EMBEDDING_DIM;
    use crate::knowledge::domain::{
        Chunk, ChunkContent, ChunkContext, ChunkSource, Embedding, HeadingText, IndexMode,
        ItemName, LineCount, LineNumber, LineRange, MarkdownContext, RepoName, RustDocContext,
        Signature, TokenCount, Visibility,
    };
    use crate::knowledge::domain::{EmbeddingModelConfig, IndexData, Timestamp};
    use std::collections::BTreeMap;
    use std::time::SystemTime;
    use tempfile::NamedTempFile;
    use test_case::test_case;

    fn test_config() -> EmbeddingModelConfig {
        EmbeddingModelConfig::default()
    }

    fn create_test_chunk_fast(file_path: &str, repo_name: &str) -> IndexedChunk {
        let chunk = Chunk::builder()
            .id(ChunkId::new(uuid::Uuid::new_v4()))
            .source(
                ChunkSource::builder()
                    .file_path(ForestRelativePath::try_new(file_path).unwrap())
                    .repo_name(RepoName::try_new(repo_name).unwrap())
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
            .build();

        IndexedChunk::builder()
            .chunk(chunk)
            .index_data(IndexData::Fast {
                bm25_terms: BTreeMap::new(),
            })
            .indexed_at(Timestamp::now())
            .build()
    }

    fn create_test_chunk_best(file_path: &str, repo_name: &str) -> IndexedChunk {
        let chunk = Chunk::builder()
            .id(ChunkId::new(uuid::Uuid::new_v4()))
            .source(
                ChunkSource::builder()
                    .file_path(ForestRelativePath::try_new(file_path).unwrap())
                    .repo_name(RepoName::try_new(repo_name).unwrap())
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
            .build();

        IndexedChunk::builder()
            .chunk(chunk)
            .index_data(IndexData::Best {
                embedding: Embedding::try_new(vec![0.1; EMBEDDING_DIM]).unwrap(),
            })
            .indexed_at(Timestamp::now())
            .build()
    }

    fn create_test_chunk() -> IndexedChunk {
        create_test_chunk_fast("test.md", "test-repo")
    }

    #[test]
    fn test_save_and_retrieve() {
        // Given A repository and a chunk
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path(), &test_config()).unwrap();
        let indexed_chunk = create_test_chunk();

        // When Saving the chunk
        repo.save(&indexed_chunk).unwrap();

        // Then It should be retrievable
        let retrieved = repo.find_by_id(&indexed_chunk.chunk.id).unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.chunk.id, indexed_chunk.chunk.id);
        assert_eq!(
            retrieved.chunk.content.text,
            indexed_chunk.chunk.content.text
        );
    }

    #[test]
    fn test_save_batch() {
        // Given A repository and multiple chunks
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path(), &test_config()).unwrap();
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
        let mut repo = SqliteChunkRepository::open(temp_file.path(), &test_config()).unwrap();
        let chunk1 = create_test_chunk_fast("test.md", "test-repo");
        let chunk2 = create_test_chunk_fast("other.md", "test-repo");

        repo.save(&chunk1).unwrap();
        repo.save(&chunk2).unwrap();

        // When Finding by file
        let results = repo
            .find_by_file(&ForestRelativePath::try_new("test.md").unwrap())
            .unwrap();

        // Then Only chunks from that file should be returned
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].chunk.id, chunk1.chunk.id);
    }

    #[test]
    fn test_delete_by_file() {
        // Given A repository with chunks
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path(), &test_config()).unwrap();
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
        let mut repo = SqliteChunkRepository::open(temp_file.path(), &test_config()).unwrap();
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
        let mut repo = SqliteChunkRepository::open(temp_file.path(), &test_config()).unwrap();

        // When Setting metadata
        let metadata = IndexMetadata::builder()
            .mode(IndexMode::Best)
            .last_build(SystemTime::now())
            .chunk_count(10)
            .file_count(5)
            .model_config(crate::knowledge::domain::EmbeddingModelConfig::default())
            .build();

        repo.set_metadata(&metadata).unwrap();

        // Then It should be retrievable
        let retrieved = repo.get_metadata().unwrap();
        assert_eq!(retrieved.mode, IndexMode::Best);
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
                .item_name(ItemName::try_new("Config").unwrap())
                .visibility(Visibility::Private)
                .build()
        )
        ; "rustdoc context without signature"
    )]
    fn test_context_roundtrip(context: ChunkContext) {
        // Given A repository and a chunk with specific context
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path(), &test_config()).unwrap();

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
            .build();

        let indexed_chunk = IndexedChunk::builder()
            .chunk(chunk)
            .index_data(IndexData::Fast {
                bm25_terms: BTreeMap::new(),
            })
            .indexed_at(Timestamp::now())
            .build();

        // When Saving and retrieving the chunk
        repo.save(&indexed_chunk).unwrap();
        let retrieved = repo.find_by_id(&indexed_chunk.chunk.id).unwrap().unwrap();

        // Then The context should be preserved with correct type
        assert_eq!(retrieved.chunk.context, context);
    }

    #[test]
    fn test_save_with_embedding_stores_in_both_tables() {
        // Given A repository and a chunk with an embedding
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path(), &test_config()).unwrap();

        let embedding = Embedding::try_new(
            (0..EMBEDDING_DIM)
                .map(|i| i as f32 / EMBEDDING_DIM as f32)
                .collect(),
        )
        .unwrap();
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
            .build();

        let indexed_chunk = IndexedChunk::builder()
            .chunk(chunk)
            .index_data(IndexData::Best { embedding })
            .indexed_at(Timestamp::now())
            .build();

        // When Saving the chunk
        repo.save(&indexed_chunk).unwrap();

        // Then It should be in both chunks and vec_chunks tables
        let chunk_exists: bool = repo
            .conn
            .query_row(
                "SELECT 1 FROM chunks WHERE id = ?1",
                rusqlite::params![indexed_chunk.chunk.id.to_string()],
                |_| Ok(true),
            )
            .unwrap();
        assert!(chunk_exists);

        let vec_chunk_exists: bool = repo
            .conn
            .query_row(
                "SELECT 1 FROM vec_chunks WHERE chunk_id = ?1",
                rusqlite::params![indexed_chunk.chunk.id.to_string()],
                |_| Ok(true),
            )
            .unwrap();
        assert!(vec_chunk_exists);
    }

    #[test]
    fn test_save_without_embedding_skips_vec_chunks() {
        // Given A repository and a chunk without an embedding
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path(), &test_config()).unwrap();

        let indexed_chunk = create_test_chunk();

        // When Saving the chunk
        repo.save(&indexed_chunk).unwrap();

        // Then It should be in chunks but not vec_chunks
        let chunk_exists: bool = repo
            .conn
            .query_row(
                "SELECT 1 FROM chunks WHERE id = ?1",
                rusqlite::params![indexed_chunk.chunk.id.to_string()],
                |_| Ok(true),
            )
            .unwrap();
        assert!(chunk_exists);

        let vec_chunk_result: Result<bool, _> = repo.conn.query_row(
            "SELECT 1 FROM vec_chunks WHERE chunk_id = ?1",
            rusqlite::params![indexed_chunk.chunk.id.to_string()],
            |_| Ok(true),
        );
        assert!(vec_chunk_result.is_err());
    }

    #[test]
    fn test_save_batch_with_embeddings() {
        // Given A repository and chunks with embeddings
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path(), &test_config()).unwrap();

        let indexed_chunk1 = create_test_chunk_best("test1.md", "test-repo");
        let indexed_chunk2 = create_test_chunk_best("test2.md", "test-repo");

        // When Saving in batch
        repo.save_batch(&[indexed_chunk1, indexed_chunk2]).unwrap();

        // Then Both should be in vec_chunks
        let count: i64 = repo
            .conn
            .query_row("SELECT COUNT(*) FROM vec_chunks", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_best_mode_roundtrip() {
        // Given A repository and a Best mode chunk with embedding
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path(), &test_config()).unwrap();

        let embedding = Embedding::try_new(
            (0..EMBEDDING_DIM)
                .map(|i| i as f32 / EMBEDDING_DIM as f32)
                .collect(),
        )
        .unwrap();

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
            .build();

        let indexed_chunk = IndexedChunk::builder()
            .chunk(chunk)
            .index_data(IndexData::Best {
                embedding: embedding.clone(),
            })
            .indexed_at(Timestamp::now())
            .build();

        // When Saving and retrieving
        repo.save(&indexed_chunk).unwrap();
        let retrieved = repo.find_by_id(&indexed_chunk.chunk.id).unwrap().unwrap();

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
        let mut repo = SqliteChunkRepository::open(temp_file.path(), &test_config()).unwrap();

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
            .build();

        let indexed_chunk = IndexedChunk::builder()
            .chunk(chunk)
            .index_data(IndexData::Fast {
                bm25_terms: BTreeMap::new(),
            })
            .indexed_at(Timestamp::now())
            .build();

        // When Saving and retrieving
        repo.save(&indexed_chunk).unwrap();
        let retrieved = repo.find_by_id(&indexed_chunk.chunk.id).unwrap().unwrap();

        // Then Empty terms are preserved
        match retrieved.index_data {
            IndexData::Fast { bm25_terms } => {
                assert!(bm25_terms.is_empty());
            }
            _ => panic!("Expected Fast mode"),
        }
    }

    #[test]
    fn test_semantic_search_finds_similar_chunks() {
        // Given A repository with saved chunks
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path(), &test_config()).unwrap();

        // Create chunks with embeddings in different directions
        // embedding1: mostly positive values (similar to query)
        let mut vec1 = vec![0.8; EMBEDDING_DIM];
        vec1[0] = 0.9; // Slightly different
        let embedding1 = Embedding::try_new(vec1).unwrap();

        // embedding2: mix of positive and negative (different direction)
        let mut vec2 = vec![0.5; EMBEDDING_DIM / 2];
        vec2.extend(vec![-0.5; EMBEDDING_DIM / 2]);
        let embedding2 = Embedding::try_new(vec2).unwrap();

        let chunk1 = Chunk::builder()
            .id(ChunkId::new(uuid::Uuid::new_v4()))
            .source(
                ChunkSource::builder()
                    .file_path(ForestRelativePath::try_new("doc1.md").unwrap())
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
                    .text("kubernetes deployment")
                    .token_count(TokenCount::try_new(2).unwrap())
                    .build(),
            )
            .context(ChunkContext::Markdown(
                MarkdownContext::builder().heading_hierarchy(vec![]).build(),
            ))
            .build();

        let chunk2 = Chunk::builder()
            .id(ChunkId::new(uuid::Uuid::new_v4()))
            .source(
                ChunkSource::builder()
                    .file_path(ForestRelativePath::try_new("doc2.md").unwrap())
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
                    .text("unrelated content")
                    .token_count(TokenCount::try_new(2).unwrap())
                    .build(),
            )
            .context(ChunkContext::Markdown(
                MarkdownContext::builder().heading_hierarchy(vec![]).build(),
            ))
            .build();

        let indexed1 = IndexedChunk::builder()
            .chunk(chunk1)
            .embedding(embedding1)
            .indexed_at(Timestamp::now())
            .build();

        let indexed2 = IndexedChunk::builder()
            .chunk(chunk2)
            .embedding(embedding2)
            .indexed_at(Timestamp::now())
            .build();

        repo.save(&indexed1).unwrap();
        repo.save(&indexed2).unwrap();

        // When Searching with an embedding similar in direction to chunk1
        let query = vec![0.85; EMBEDDING_DIM]; // All positive, similar to embedding1
        let results = repo.search_semantic(&query, 10).unwrap();

        // Then Both chunks are found, with chunk1 ranked higher (more similar direction)
        assert_eq!(results.len(), 2);
        assert_eq!(
            results[0].0.chunk.id, indexed1.chunk.id,
            "Chunk with similar direction should rank first"
        );
        assert!(
            results[0].1 > results[1].1,
            "First result should have higher similarity score"
        );
    }

    #[test]
    fn test_best_mode_zero_embedding() {
        // Given An attempt to create an empty embedding
        let empty_embedding = Embedding::try_new(vec![]);

        // When Creating the embedding
        // Then It should fail validation at the type level
        assert!(empty_embedding.is_err());
    }

    #[test]
    fn test_delete_by_file_removes_embeddings() {
        // Given A repository with Best mode chunks containing embeddings
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path(), &test_config()).unwrap();

        let chunk = create_test_chunk_best("test.md", "test-repo");
        repo.save(&chunk).unwrap();

        let vec_count_before: i64 = repo
            .conn
            .query_row("SELECT COUNT(*) FROM vec_chunks", [], |row| row.get(0))
            .unwrap();
        assert_eq!(vec_count_before, 1);

        // When Deleting chunks by file
        repo.delete_by_file(&ForestRelativePath::try_new("test.md").unwrap())
            .unwrap();

        // Then Embeddings should also be deleted from vec_chunks
        let vec_count_after: i64 = repo
            .conn
            .query_row("SELECT COUNT(*) FROM vec_chunks", [], |row| row.get(0))
            .unwrap();
        assert_eq!(vec_count_after, 0);
    }

    #[test]
    fn test_clear_removes_all_embeddings() {
        // Given A repository with multiple Best mode chunks
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path(), &test_config()).unwrap();

        let chunk1 = create_test_chunk_best("file1.md", "repo1");
        let chunk2 = create_test_chunk_best("file2.md", "repo2");
        repo.save(&chunk1).unwrap();
        repo.save(&chunk2).unwrap();

        let vec_count_before: i64 = repo
            .conn
            .query_row("SELECT COUNT(*) FROM vec_chunks", [], |row| row.get(0))
            .unwrap();
        assert_eq!(vec_count_before, 2);

        // When Clearing all chunks
        repo.clear().unwrap();

        // Then All embeddings should be deleted from vec_chunks
        let vec_count_after: i64 = repo
            .conn
            .query_row("SELECT COUNT(*) FROM vec_chunks", [], |row| row.get(0))
            .unwrap();
        assert_eq!(vec_count_after, 0);
    }

    #[test]
    fn test_delete_by_file_with_mixed_modes() {
        // Given A repository with both Fast and Best mode chunks for the same file
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path(), &test_config()).unwrap();

        let chunk_fast = create_test_chunk_fast("test.md", "test-repo");
        let chunk_best = create_test_chunk_best("test.md", "test-repo");
        repo.save(&chunk_fast).unwrap();
        repo.save(&chunk_best).unwrap();

        let vec_count_before: i64 = repo
            .conn
            .query_row("SELECT COUNT(*) FROM vec_chunks", [], |row| row.get(0))
            .unwrap();
        assert_eq!(vec_count_before, 1);

        // When Deleting chunks by file
        repo.delete_by_file(&ForestRelativePath::try_new("test.md").unwrap())
            .unwrap();

        // Then Only the Best mode embedding should be deleted
        let vec_count_after: i64 = repo
            .conn
            .query_row("SELECT COUNT(*) FROM vec_chunks", [], |row| row.get(0))
            .unwrap();
        assert_eq!(vec_count_after, 0);
    }

    #[test]
    fn test_metadata_model_config_roundtrip() {
        use crate::knowledge::domain::EmbeddingModelConfig;

        // Given A repository with custom model config
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path(), &test_config()).unwrap();

        let custom_config = EmbeddingModelConfig::builder()
            .model_name("custom-model")
            .embedding_dim(512)
            .max_tokens(128)
            .overlap_tokens(20)
            .build();

        let metadata = IndexMetadata::builder()
            .mode(IndexMode::Best)
            .last_build(SystemTime::now())
            .chunk_count(42)
            .file_count(7)
            .model_config(custom_config.clone())
            .build();

        // When Setting and retrieving metadata
        repo.set_metadata(&metadata).unwrap();
        let retrieved = repo.get_metadata().unwrap();

        // Then All model config fields are preserved
        assert_eq!(retrieved.model_config.model_name, custom_config.model_name);
        assert_eq!(
            retrieved.model_config.embedding_dim,
            custom_config.embedding_dim
        );
        assert_eq!(retrieved.model_config.max_tokens, custom_config.max_tokens);
        assert_eq!(
            retrieved.model_config.overlap_tokens,
            custom_config.overlap_tokens
        );
    }

    #[test]
    fn test_metadata_default_model_config() {
        use crate::knowledge::domain::EmbeddingModelConfig;

        // Given A repository with default model config
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path(), &test_config()).unwrap();

        let metadata = IndexMetadata::builder()
            .mode(IndexMode::Fast)
            .last_build(SystemTime::now())
            .chunk_count(0)
            .file_count(0)
            .model_config(EmbeddingModelConfig::default())
            .build();

        // When Setting and retrieving metadata
        repo.set_metadata(&metadata).unwrap();
        let retrieved = repo.get_metadata().unwrap();

        // Then Default config values are preserved
        assert_eq!(
            retrieved.model_config.model_name,
            "sentence-transformers/all-MiniLM-L6-v2"
        );
        assert_eq!(retrieved.model_config.embedding_dim, 384);
        assert_eq!(retrieved.model_config.max_tokens, 256);
        assert_eq!(retrieved.model_config.overlap_tokens, 38);
    }

    #[test]
    fn test_metadata_persists_across_reopens() {
        use crate::knowledge::domain::EmbeddingModelConfig;

        // Given A repository with metadata
        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path().to_path_buf();

        let custom_config = EmbeddingModelConfig::builder()
            .model_name("test-model")
            .embedding_dim(256)
            .max_tokens(512)
            .overlap_tokens(64)
            .build();

        {
            let mut repo = SqliteChunkRepository::open(&temp_path, &test_config()).unwrap();
            let metadata = IndexMetadata::builder()
                .mode(IndexMode::Best)
                .last_build(SystemTime::now())
                .chunk_count(100)
                .file_count(10)
                .model_config(custom_config.clone())
                .build();

            repo.set_metadata(&metadata).unwrap();
        }

        // When Reopening the database
        let repo = SqliteChunkRepository::open(&temp_path, &test_config()).unwrap();
        let retrieved = repo.get_metadata().unwrap();

        // Then Model config is still present
        assert_eq!(retrieved.model_config, custom_config);
    }

    #[test]
    fn test_open_with_matching_config_succeeds() {
        use crate::knowledge::domain::EmbeddingModelConfig;

        // Given An existing index with specific model config
        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path().to_path_buf();

        let config = EmbeddingModelConfig::builder()
            .model_name("test-model")
            .embedding_dim(256)
            .max_tokens(512)
            .overlap_tokens(64)
            .build();

        {
            let mut repo = SqliteChunkRepository::open(&temp_path, &test_config()).unwrap();
            let metadata = IndexMetadata::builder()
                .mode(IndexMode::Best)
                .last_build(SystemTime::now())
                .chunk_count(0)
                .file_count(0)
                .model_config(config.clone())
                .build();
            repo.set_metadata(&metadata).unwrap();
        }

        // When Opening with matching config
        let result = SqliteChunkRepository::open_with_config(&temp_path, &config);

        // Then It should succeed
        assert!(result.is_ok());
    }

    #[test]
    fn test_open_with_mismatched_model_name_fails() {
        use crate::knowledge::domain::EmbeddingModelConfig;

        // Given An existing index with one model
        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path().to_path_buf();

        let stored_config = EmbeddingModelConfig::builder()
            .model_name("model-a")
            .embedding_dim(384)
            .max_tokens(256)
            .overlap_tokens(38)
            .build();

        {
            let mut repo = SqliteChunkRepository::open(&temp_path, &test_config()).unwrap();
            let metadata = IndexMetadata::builder()
                .mode(IndexMode::Best)
                .last_build(SystemTime::now())
                .chunk_count(0)
                .file_count(0)
                .model_config(stored_config)
                .build();
            repo.set_metadata(&metadata).unwrap();
        }

        // When Opening with different model name
        let expected_config = EmbeddingModelConfig::builder()
            .model_name("model-b")
            .embedding_dim(384)
            .max_tokens(256)
            .overlap_tokens(38)
            .build();

        let result = SqliteChunkRepository::open_with_config(&temp_path, &expected_config);

        // Then It should fail with ConfigMismatch error
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, StorageError::ConfigMismatch { .. }));
    }

    #[test]
    fn test_open_with_mismatched_embedding_dim_fails() {
        use crate::knowledge::domain::EmbeddingModelConfig;

        // Given An existing index with one embedding dimension
        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path().to_path_buf();

        let stored_config = EmbeddingModelConfig::builder()
            .model_name("test-model")
            .embedding_dim(384)
            .max_tokens(256)
            .overlap_tokens(38)
            .build();

        {
            let mut repo = SqliteChunkRepository::open(&temp_path, &test_config()).unwrap();
            let metadata = IndexMetadata::builder()
                .mode(IndexMode::Best)
                .last_build(SystemTime::now())
                .chunk_count(0)
                .file_count(0)
                .model_config(stored_config)
                .build();
            repo.set_metadata(&metadata).unwrap();
        }

        // When Opening with different embedding dimension
        let expected_config = EmbeddingModelConfig::builder()
            .model_name("test-model")
            .embedding_dim(512)
            .max_tokens(256)
            .overlap_tokens(38)
            .build();

        let result = SqliteChunkRepository::open_with_config(&temp_path, &expected_config);

        // Then It should fail with ConfigMismatch error
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StorageError::ConfigMismatch { .. }
        ));
    }

    #[test]
    fn test_open_with_mismatched_max_tokens_fails() {
        use crate::knowledge::domain::EmbeddingModelConfig;

        // Given An existing index with one max_tokens value
        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path().to_path_buf();

        let stored_config = EmbeddingModelConfig::builder()
            .model_name("test-model")
            .embedding_dim(384)
            .max_tokens(256)
            .overlap_tokens(38)
            .build();

        {
            let mut repo = SqliteChunkRepository::open(&temp_path, &test_config()).unwrap();
            let metadata = IndexMetadata::builder()
                .mode(IndexMode::Best)
                .last_build(SystemTime::now())
                .chunk_count(0)
                .file_count(0)
                .model_config(stored_config)
                .build();
            repo.set_metadata(&metadata).unwrap();
        }

        // When Opening with different max_tokens
        let expected_config = EmbeddingModelConfig::builder()
            .model_name("test-model")
            .embedding_dim(384)
            .max_tokens(512)
            .overlap_tokens(38)
            .build();

        let result = SqliteChunkRepository::open_with_config(&temp_path, &expected_config);

        // Then It should fail with ConfigMismatch error
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StorageError::ConfigMismatch { .. }
        ));
    }

    #[test]
    fn test_open_with_mismatched_overlap_tokens_fails() {
        use crate::knowledge::domain::EmbeddingModelConfig;

        // Given An existing index with one overlap_tokens value
        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path().to_path_buf();

        let stored_config = EmbeddingModelConfig::builder()
            .model_name("test-model")
            .embedding_dim(384)
            .max_tokens(256)
            .overlap_tokens(38)
            .build();

        {
            let mut repo = SqliteChunkRepository::open(&temp_path, &test_config()).unwrap();
            let metadata = IndexMetadata::builder()
                .mode(IndexMode::Best)
                .last_build(SystemTime::now())
                .chunk_count(0)
                .file_count(0)
                .model_config(stored_config)
                .build();
            repo.set_metadata(&metadata).unwrap();
        }

        // When Opening with different overlap_tokens
        let expected_config = EmbeddingModelConfig::builder()
            .model_name("test-model")
            .embedding_dim(384)
            .max_tokens(256)
            .overlap_tokens(64)
            .build();

        let result = SqliteChunkRepository::open_with_config(&temp_path, &expected_config);

        // Then It should fail with ConfigMismatch error
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StorageError::ConfigMismatch { .. }
        ));
    }

    #[test]
    fn test_config_mismatch_error_message_includes_details() {
        use crate::knowledge::domain::EmbeddingModelConfig;

        // Given An existing index with specific config
        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path().to_path_buf();

        let stored_config = EmbeddingModelConfig::builder()
            .model_name("old-model")
            .embedding_dim(256)
            .max_tokens(128)
            .overlap_tokens(20)
            .build();

        {
            let mut repo = SqliteChunkRepository::open(&temp_path, &test_config()).unwrap();
            let metadata = IndexMetadata::builder()
                .mode(IndexMode::Best)
                .last_build(SystemTime::now())
                .chunk_count(0)
                .file_count(0)
                .model_config(stored_config)
                .build();
            repo.set_metadata(&metadata).unwrap();
        }

        // When Opening with completely different config
        let expected_config = EmbeddingModelConfig::builder()
            .model_name("new-model")
            .embedding_dim(512)
            .max_tokens(256)
            .overlap_tokens(40)
            .build();

        let result = SqliteChunkRepository::open_with_config(&temp_path, &expected_config);

        // Then Error message should contain expected and actual values
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("old-model") || err_msg.contains("new-model"));
    }

    #[test]
    fn test_open_without_config_validation_still_works() {
        use crate::knowledge::domain::EmbeddingModelConfig;

        // Given An existing index with any config
        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path().to_path_buf();

        {
            let mut repo = SqliteChunkRepository::open(&temp_path, &test_config()).unwrap();
            let metadata = IndexMetadata::builder()
                .mode(IndexMode::Best)
                .last_build(SystemTime::now())
                .chunk_count(0)
                .file_count(0)
                .model_config(EmbeddingModelConfig::default())
                .build();
            repo.set_metadata(&metadata).unwrap();
        }

        // When Opening without config validation
        let result = SqliteChunkRepository::open(&temp_path, &test_config());

        // Then It should succeed regardless of stored config
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_indexed_files() {
        // Given A repository with multiple chunks from different files
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path(), &test_config()).unwrap();

        let chunk1 = create_test_chunk_fast("repo1/file1.md", "repo1");
        let chunk2 = create_test_chunk_fast("repo1/file1.md", "repo1");
        let chunk3 = create_test_chunk_fast("repo2/file2.md", "repo2");

        repo.save(&chunk1).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        repo.save(&chunk2).unwrap();
        repo.save(&chunk3).unwrap();

        // When Getting indexed files
        let indexed_files = repo.get_indexed_files().unwrap();

        // Then It should return one entry per file with the latest timestamp
        assert_eq!(indexed_files.len(), 2);
        assert!(
            indexed_files.contains_key(&ForestRelativePath::try_new("repo1/file1.md").unwrap())
        );
        assert!(
            indexed_files.contains_key(&ForestRelativePath::try_new("repo2/file2.md").unwrap())
        );

        let file1_ts = indexed_files
            .get(&ForestRelativePath::try_new("repo1/file1.md").unwrap())
            .unwrap();
        let file2_ts = indexed_files
            .get(&ForestRelativePath::try_new("repo2/file2.md").unwrap())
            .unwrap();

        assert!(file1_ts >= file2_ts);
    }
}
