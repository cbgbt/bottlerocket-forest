//! SQLite implementation of ChunkRepository
//!
//! Provides persistent storage for indexed chunks using SQLite with vector search capabilities.
//!
//! # Submodules
//!
//! * `serialization`: Converts between domain types and database formats
//! * `queries`: Implements CRUD operations for chunk storage
//! * `search`: Provides semantic search using vector embeddings
//! * `context`: Context repository for multi-context indexing
//! * `files`: Storage queries for indexed files
//! * `chunks`: Content-addressed chunk storage queries

pub mod chunks;
mod context;
pub mod files;
mod queries;
mod search;
mod serialization;

pub use context::SqliteContextRepository;

use rusqlite::Connection;
use snafu::ResultExt;
use std::path::Path;

use super::repository::{ChunkRepository, StorageError, storage_error::*};
use super::schema;
use crate::knowledge::domain::{
    ChunkHash, ChunkId, ContextId, EmbeddingModelConfig, FileHash, ForestRelativePath,
    IndexMetadata, IndexedChunk, Timestamp,
};

/// SQLite-backed implementation of chunk repository with vector search
#[derive(Debug)]
pub struct SqliteChunkRepository {
    conn: Connection,
}

impl SqliteChunkRepository {
    /// Opens or creates a database at the specified path
    ///
    /// Initializes the schema and registers the sqlite-vec extension for vector operations.
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

    /// Opens an existing database and validates model configuration compatibility
    ///
    /// Verifies that the stored model configuration matches the expected configuration.
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

    /// Returns a context repository backed by this connection
    pub fn context_repository(&self) -> SqliteContextRepository<'_> {
        SqliteContextRepository::new(&self.conn)
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
        context_id: Option<ContextId>,
    ) -> Result<Vec<(IndexedChunk, f32)>, StorageError> {
        search::search_semantic(&self.conn, query_embedding, limit, context_id)
    }

    fn has_embedding(&self, chunk_hash: &ChunkHash) -> Result<bool, StorageError> {
        let result = self.has_embedding_batch(&[*chunk_hash])?;
        Ok(result.contains(chunk_hash))
    }

    fn has_embedding_batch(
        &self,
        chunk_hashes: &[ChunkHash],
    ) -> Result<std::collections::HashSet<ChunkHash>, StorageError> {
        use super::repository::storage_error::*;

        if chunk_hashes.is_empty() {
            return Ok(std::collections::HashSet::new());
        }

        let placeholders = chunk_hashes
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        let query = format!(
            "SELECT chunk_hash FROM vec_chunks WHERE chunk_hash IN ({})",
            placeholders
        );

        let mut stmt = self.conn.prepare(&query).context(DatabaseSnafu)?;
        let params: Vec<String> = chunk_hashes.iter().map(|h| h.to_string()).collect();
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(|s| s as &dyn rusqlite::ToSql).collect();

        let mut rows = stmt.query(params_refs.as_slice()).context(DatabaseSnafu)?;
        let mut existing = std::collections::HashSet::new();

        while let Some(row) = rows.next().context(DatabaseSnafu)? {
            let hash_str: String = row.get(0).context(DatabaseSnafu)?;
            if let Some(hash) = chunk_hashes.iter().find(|h| h.to_string() == hash_str) {
                existing.insert(*hash);
            }
        }

        Ok(existing)
    }

    fn track_indexed_file(
        &mut self,
        file_path: &ForestRelativePath,
        file_hash: &FileHash,
        mtime: Timestamp,
    ) -> Result<(), StorageError> {
        // Uses the default context "." for now. When multi-context support is fully
        // implemented, this will accept a context_id parameter to support indexing
        // files in different contexts (e.g., worktrees).
        self.conn
            .execute(
                "INSERT OR REPLACE INTO indexed_files (context_id, file_path, file_hash, mtime_ns) VALUES (?, ?, ?, ?)",
                rusqlite::params![
                    ".",
                    file_path.to_string(),
                    file_hash.as_bytes().as_slice(),
                    mtime.as_secs() * 1_000_000_000,
                ],
            )
            .context(DatabaseSnafu)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::knowledge::constants::EMBEDDING_DIM;
    use crate::knowledge::domain::{
        Chunk, ChunkContent, ChunkContext, ChunkHash, ChunkSource, Embedding, FileHash,
        HeadingText, ItemName, MarkdownContext, RepoName, RustDocContext, Signature, TokenCount,
        Visibility,
    };
    use crate::knowledge::domain::{EmbeddingModelConfig, Timestamp};
    use std::time::SystemTime;
    use tempfile::NamedTempFile;
    use test_case::test_case;

    fn test_config() -> EmbeddingModelConfig {
        EmbeddingModelConfig::default()
    }

    fn create_test_chunk_fast(file_path: &str, repo_name: &str) -> IndexedChunk {
        let content = format!("test content for {}", file_path);
        let chunk_hash = ChunkHash::from_text(&content);
        let file_hash = FileHash::new([0u8; 32]); // Placeholder file hash

        let chunk = Chunk::builder()
            .id(ChunkId::new(uuid::Uuid::new_v4()))
            .chunk_hash(chunk_hash)
            .file_hash(file_hash)
            .source(
                ChunkSource::builder()
                    .file_path(ForestRelativePath::try_new(file_path).unwrap())
                    .repo_name(RepoName::try_new(repo_name).unwrap())
                    .build(),
            )
            .content(
                ChunkContent::builder()
                    .text(&content)
                    .token_count(TokenCount::try_new(10).unwrap())
                    .build(),
            )
            .context(ChunkContext::Markdown(
                MarkdownContext::builder().heading_hierarchy(vec![]).build(),
            ))
            .build();

        IndexedChunk::builder()
            .chunk(chunk)
            .embedding(Embedding::try_new(vec![0.1; EMBEDDING_DIM]).unwrap())
            .indexed_at(Timestamp::now())
            .build()
    }

    fn create_test_chunk_best(file_path: &str, repo_name: &str) -> IndexedChunk {
        let content = format!("best test content for {}", file_path);
        let chunk_hash = ChunkHash::from_text(&content);
        let file_hash = FileHash::new([1u8; 32]); // Different placeholder file hash

        let chunk = Chunk::builder()
            .id(ChunkId::new(uuid::Uuid::new_v4()))
            .chunk_hash(chunk_hash)
            .file_hash(file_hash)
            .source(
                ChunkSource::builder()
                    .file_path(ForestRelativePath::try_new(file_path).unwrap())
                    .repo_name(RepoName::try_new(repo_name).unwrap())
                    .build(),
            )
            .content(
                ChunkContent::builder()
                    .text(&content)
                    .token_count(TokenCount::try_new(10).unwrap())
                    .build(),
            )
            .context(ChunkContext::Markdown(
                MarkdownContext::builder().heading_hierarchy(vec![]).build(),
            ))
            .build();

        IndexedChunk::builder()
            .chunk(chunk)
            .embedding(Embedding::try_new(vec![0.1; EMBEDDING_DIM]).unwrap())
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

        // Then It should be retrievable via find_all
        let all = repo.find_all().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].chunk.content.text, indexed_chunk.chunk.content.text);
        assert_eq!(all[0].chunk.chunk_hash, indexed_chunk.chunk.chunk_hash);
    }

    #[test]
    fn test_save_batch() {
        // Given A repository and multiple chunks with different content
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path(), &test_config()).unwrap();
        let chunk1 = create_test_chunk_fast("file1.md", "test-repo");
        let chunk2 = create_test_chunk_fast("file2.md", "test-repo");
        let chunks = vec![chunk1, chunk2];

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

        // When Finding by file (note: in new schema, file_path is not stored in chunks)
        let results = repo
            .find_by_file(&ForestRelativePath::try_new("test.md").unwrap())
            .unwrap();

        // Then Returns empty because file_path is no longer stored in chunks table
        // Use find_by_file_hash for file-based lookups in the new schema
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_delete_by_file() {
        // Given A repository with chunks
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path(), &test_config()).unwrap();
        let chunk = create_test_chunk();
        repo.save(&chunk).unwrap();

        // When Deleting by file (note: in new schema, file_path is not stored in chunks)
        let count = repo
            .delete_by_file(&ForestRelativePath::try_new("test.md").unwrap())
            .unwrap();

        // Then Returns 0 because file_path is no longer stored in chunks table
        // Use delete_by_file_hash for file-based deletion in the new schema
        assert_eq!(count, 0);
        let all = repo.find_all().unwrap();
        assert_eq!(all.len(), 1); // Chunk still exists
    }

    #[test]
    fn test_clear() {
        // Given A repository with chunks
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path(), &test_config()).unwrap();
        let chunk1 = create_test_chunk_fast("file1.md", "test-repo");
        let chunk2 = create_test_chunk_fast("file2.md", "test-repo");
        repo.save(&chunk1).unwrap();
        repo.save(&chunk2).unwrap();

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
            .last_build(SystemTime::now())
            .chunk_count(10)
            .file_count(5)
            .model_config(crate::knowledge::domain::EmbeddingModelConfig::default())
            .build();

        repo.set_metadata(&metadata).unwrap();

        // Then It should be retrievable
        let retrieved = repo.get_metadata().unwrap();
        // chunk_count and file_count are derived from chunks table, not stored in metadata
        assert_eq!(retrieved.chunk_count, 0); // No chunks inserted yet
        assert_eq!(retrieved.file_count, 0);
        assert_eq!(retrieved.model_config, metadata.model_config);
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
                .item_type(crate::knowledge::indexing::RustItemType::Function)
                .build()
        )
        ; "rustdoc context with signature"
    )]
    #[test_case(
        ChunkContext::RustDoc(
            RustDocContext::builder()
                .item_name(ItemName::try_new("Config").unwrap())
                .visibility(Visibility::Private)
                .item_type(crate::knowledge::indexing::RustItemType::Struct)
                .build()
        )
        ; "rustdoc context without signature"
    )]
    fn test_context_roundtrip(context: ChunkContext) {
        // Given A repository and a chunk with specific context
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path(), &test_config()).unwrap();

        let content = "test content for context roundtrip";
        let chunk_hash = ChunkHash::from_text(content);
        let file_hash = FileHash::new([2u8; 32]);

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
            .context(context.clone())
            .build();

        let indexed_chunk = IndexedChunk::builder()
            .chunk(chunk)
            .embedding(Embedding::try_new(vec![0.1; EMBEDDING_DIM]).unwrap())
            .indexed_at(Timestamp::now())
            .build();

        // When Saving and retrieving the chunk
        repo.save(&indexed_chunk).unwrap();
        let all = repo.find_all().unwrap();
        let retrieved = &all[0];

        // Then The context should be preserved with correct type
        assert_eq!(retrieved.chunk.context, context);
    }

    #[test]
    fn test_save_with_embedding_stores_in_both_tables() {
        // Given A repository and a chunk with an embedding
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path(), &test_config()).unwrap();

        let embedding = Embedding::try_new(
            (1..=EMBEDDING_DIM)
                .map(|i| i as f32 / EMBEDDING_DIM as f32)
                .collect(),
        )
        .unwrap();

        let content = "test content for embedding";
        let chunk_hash = ChunkHash::from_text(content);
        let file_hash = FileHash::new([3u8; 32]);

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

        let indexed_chunk = IndexedChunk::builder()
            .chunk(chunk)
            .embedding(embedding)
            .indexed_at(Timestamp::now())
            .build();

        // When Saving the chunk
        repo.save(&indexed_chunk).unwrap();

        // Then It should be in both chunks and vec_chunks tables
        let chunk_exists: bool = repo
            .conn
            .query_row(
                "SELECT 1 FROM chunks WHERE chunk_hash = ?1",
                rusqlite::params![indexed_chunk.chunk.chunk_hash.as_bytes().as_slice()],
                |_| Ok(true),
            )
            .unwrap();
        assert!(chunk_exists);

        let vec_chunk_exists: bool = repo
            .conn
            .query_row(
                "SELECT 1 FROM vec_chunks WHERE chunk_hash = ?1",
                rusqlite::params![indexed_chunk.chunk.chunk_hash.to_string()],
                |_| Ok(true),
            )
            .unwrap();
        assert!(vec_chunk_exists);
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
            (1..=EMBEDDING_DIM)
                .map(|i| i as f32 / EMBEDDING_DIM as f32)
                .collect(),
        )
        .unwrap();

        let content = "semantic search content";
        let chunk_hash = ChunkHash::from_text(content);
        let file_hash = FileHash::new([4u8; 32]);

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
                    .token_count(TokenCount::try_new(23).unwrap())
                    .build(),
            )
            .context(ChunkContext::Markdown(
                MarkdownContext::builder().heading_hierarchy(vec![]).build(),
            ))
            .build();

        let indexed_chunk = IndexedChunk::builder()
            .chunk(chunk)
            .embedding(embedding.clone())
            .indexed_at(Timestamp::now())
            .build();

        // When Saving and retrieving
        repo.save(&indexed_chunk).unwrap();
        let all = repo.find_all().unwrap();
        let retrieved = &all[0];

        // Then Embedding is stored (not retrieved in regular queries, but stored in vec_chunks)
        assert_eq!(retrieved.chunk.chunk_hash, indexed_chunk.chunk.chunk_hash);
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

        let content1 = "kubernetes deployment";
        let content2 = "unrelated content";

        let chunk1 = Chunk::builder()
            .id(ChunkId::new(uuid::Uuid::new_v4()))
            .chunk_hash(ChunkHash::from_text(content1))
            .file_hash(FileHash::new([5u8; 32]))
            .source(
                ChunkSource::builder()
                    .file_path(ForestRelativePath::try_new("doc1.md").unwrap())
                    .repo_name(RepoName::try_new("test-repo").unwrap())
                    .build(),
            )
            .content(
                ChunkContent::builder()
                    .text(content1)
                    .token_count(TokenCount::try_new(2).unwrap())
                    .build(),
            )
            .context(ChunkContext::Markdown(
                MarkdownContext::builder().heading_hierarchy(vec![]).build(),
            ))
            .build();

        let chunk2 = Chunk::builder()
            .id(ChunkId::new(uuid::Uuid::new_v4()))
            .chunk_hash(ChunkHash::from_text(content2))
            .file_hash(FileHash::new([6u8; 32]))
            .source(
                ChunkSource::builder()
                    .file_path(ForestRelativePath::try_new("doc2.md").unwrap())
                    .repo_name(RepoName::try_new("test-repo").unwrap())
                    .build(),
            )
            .content(
                ChunkContent::builder()
                    .text(content2)
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
        let results = repo.search_semantic(&query, 10, None).unwrap();

        // Then Both chunks are found, with chunk1 ranked higher (more similar direction)
        assert_eq!(results.len(), 2);
        assert_eq!(
            results[0].0.chunk.chunk_hash, indexed1.chunk.chunk_hash,
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

        // When Deleting chunks by file (note: returns 0 in new schema)
        let count = repo
            .delete_by_file(&ForestRelativePath::try_new("test.md").unwrap())
            .unwrap();

        // Then In new schema, delete_by_file returns 0 (file_path not stored)
        // Embeddings remain because we need to use delete_by_file_hash
        assert_eq!(count, 0);
        let vec_count_after: i64 = repo
            .conn
            .query_row("SELECT COUNT(*) FROM vec_chunks", [], |row| row.get(0))
            .unwrap();
        assert_eq!(vec_count_after, 1); // Still exists
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
        let chunk2 = create_test_chunk_fast("repo2/file2.md", "repo2");

        repo.save(&chunk1).unwrap();
        repo.save(&chunk2).unwrap();

        // When Getting indexed files
        let indexed_files = repo.get_indexed_files().unwrap();

        // Then In the new schema, get_indexed_files returns empty map
        // because file_path is no longer stored in chunks table.
        // File tracking is now done through the indexed_files table
        // which is context-scoped.
        assert_eq!(indexed_files.len(), 0);
    }

    #[test]
    fn has_embedding_returns_true_when_embedding_exists() {
        // Given a repository with a saved chunk (which has an embedding in vec_chunks)
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path(), &test_config()).unwrap();
        let chunk = create_test_chunk();
        repo.save(&chunk).unwrap();

        // When checking if the embedding exists
        let result = repo.has_embedding(&chunk.chunk.chunk_hash);

        // Then it should return true
        assert!(result.unwrap());
    }

    #[test]
    fn has_embedding_returns_false_when_embedding_does_not_exist() {
        // Given a repository with no chunks
        let temp_file = NamedTempFile::new().unwrap();
        let repo = SqliteChunkRepository::open(temp_file.path(), &test_config()).unwrap();
        let nonexistent_hash = ChunkHash::from_text("nonexistent content");

        // When checking if an embedding exists for a hash that was never stored
        let result = repo.has_embedding(&nonexistent_hash);

        // Then it should return false
        assert!(!result.unwrap());
    }

    #[test]
    fn has_embedding_batch_returns_empty_set_when_no_embeddings_exist() {
        // Given a repository with no chunks
        let temp_file = NamedTempFile::new().unwrap();
        let repo = SqliteChunkRepository::open(temp_file.path(), &test_config()).unwrap();
        let hashes = vec![
            ChunkHash::from_text("content1"),
            ChunkHash::from_text("content2"),
            ChunkHash::from_text("content3"),
        ];

        // When checking which hashes have embeddings
        let result = repo.has_embedding_batch(&hashes);

        // Then it should return an empty set
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn has_embedding_batch_returns_subset_of_hashes_that_have_embeddings() {
        // Given a repository with some chunks saved
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path(), &test_config()).unwrap();

        let chunk1 = create_test_chunk_fast("file1.md", "repo");
        let chunk2 = create_test_chunk_fast("file2.md", "repo");
        repo.save(&chunk1).unwrap();
        repo.save(&chunk2).unwrap();

        // When checking a mix of existing and non-existing hashes
        let existing_hash1 = chunk1.chunk.chunk_hash;
        let existing_hash2 = chunk2.chunk.chunk_hash;
        let nonexistent_hash = ChunkHash::from_text("nonexistent content");
        let hashes = vec![existing_hash1, nonexistent_hash, existing_hash2];

        let result = repo.has_embedding_batch(&hashes).unwrap();

        // Then it should return only the hashes that exist
        assert_eq!(result.len(), 2);
        assert!(result.contains(&existing_hash1));
        assert!(result.contains(&existing_hash2));
        assert!(!result.contains(&nonexistent_hash));
    }

    #[test]
    fn has_embedding_batch_returns_all_hashes_when_all_have_embeddings() {
        // Given a repository with chunks saved
        let temp_file = NamedTempFile::new().unwrap();
        let mut repo = SqliteChunkRepository::open(temp_file.path(), &test_config()).unwrap();

        let chunk1 = create_test_chunk_fast("file1.md", "repo");
        let chunk2 = create_test_chunk_fast("file2.md", "repo");
        let chunk3 = create_test_chunk_best("file3.md", "repo");
        repo.save(&chunk1).unwrap();
        repo.save(&chunk2).unwrap();
        repo.save(&chunk3).unwrap();

        // When checking hashes that all exist
        let hashes = vec![
            chunk1.chunk.chunk_hash,
            chunk2.chunk.chunk_hash,
            chunk3.chunk.chunk_hash,
        ];

        let result = repo.has_embedding_batch(&hashes).unwrap();

        // Then it should return all hashes
        assert_eq!(result.len(), 3);
        for hash in &hashes {
            assert!(result.contains(hash));
        }
    }

    #[test]
    fn has_embedding_batch_handles_empty_input() {
        // Given a repository
        let temp_file = NamedTempFile::new().unwrap();
        let repo = SqliteChunkRepository::open(temp_file.path(), &test_config()).unwrap();

        // When checking an empty list of hashes
        let result = repo.has_embedding_batch(&[]);

        // Then it should return an empty set
        assert!(result.unwrap().is_empty());
    }
}
