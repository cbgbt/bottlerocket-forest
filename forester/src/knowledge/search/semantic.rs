//! Semantic search implementation using embeddings
//!
//! Implements the SearchEngine trait using semantic similarity search with
//! embeddings. This engine generates embeddings for query text and finds
//! chunks with similar semantic meaning using vector similarity.
//!
//! For embedding generation, see [`crate::knowledge::search::embeddings`].
//! For vector similarity search implementation, see [`crate::knowledge::storage::sqlite::search::search_semantic`].

use snafu::ResultExt;

use crate::knowledge::domain::{IndexMode, SearchQuery, SearchResults};
use crate::knowledge::storage::ChunkRepository;

use super::{EmbeddingProvider, SearchEngine, SearchError};

/// Semantic search engine for Best mode embedding-based search
///
/// Uses an embedding provider to convert query text into semantic vectors,
/// then searches for chunks with similar embeddings using vector similarity.
pub struct SemanticSearchEngine<R: ChunkRepository> {
    repository: R,
    embedding_provider: Box<dyn EmbeddingProvider>,
}

impl<R: ChunkRepository> SemanticSearchEngine<R> {
    /// Create a new semantic search engine with the given repository and embedding provider
    pub fn new(repository: R, embedding_provider: Box<dyn EmbeddingProvider>) -> Self {
        Self {
            repository,
            embedding_provider,
        }
    }
}

impl<R: ChunkRepository> SearchEngine for SemanticSearchEngine<R> {
    fn search(&self, query: &SearchQuery) -> Result<SearchResults, SearchError> {
        use super::engine::search_error::*;
        use std::time::Instant;

        if query.mode != IndexMode::Best {
            return Err(SearchError::ModeMismatch {
                query_mode: query.mode,
                engine_mode: IndexMode::Best,
            });
        }

        let start = Instant::now();

        let query_embedding = self
            .embedding_provider
            .embed(query.text.clone().into_inner().as_str())
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            .context(EmbeddingFailedSnafu)?;

        let results = self
            .repository
            .search_semantic(query_embedding.as_ref(), query.limit.into_inner())
            .context(StorageSnafu)?;

        let search_duration = start.elapsed();
        let total_chunks_searched = results.len();

        let search_results: Vec<_> = results
            .into_iter()
            .map(|(indexed_chunk, score)| {
                use crate::knowledge::domain::{RelevanceScore, SearchResult};
                let relevance_score = RelevanceScore::try_new(score)
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                    .context(InvalidScoreSnafu { score })?;

                Ok(SearchResult::builder()
                    .chunk(indexed_chunk.chunk)
                    .score(relevance_score)
                    .matched_terms(vec![])
                    .build())
            })
            .collect::<Result<Vec<_>, SearchError>>()?;

        Ok(SearchResults::builder()
            .query(query.clone())
            .results(search_results)
            .total_chunks_searched(total_chunks_searched)
            .search_duration(search_duration)
            .build())
    }

    fn mode(&self) -> IndexMode {
        IndexMode::Best
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::knowledge::domain::{
        Chunk, ChunkContent, ChunkContext, ChunkId, ChunkSource, Embedding, ForestRelativePath,
        IndexData, IndexedChunk, LineCount, LineNumber, LineRange, MarkdownContext, QueryText,
        RepoName, ResultLimit, Timestamp, TokenCount,
    };
    use crate::knowledge::search::embeddings::model::MockEmbeddingProvider;
    use crate::knowledge::storage::repository::MockChunkRepository;
    use test_case::test_case;

    fn create_test_embedding(values: Vec<f32>) -> Embedding {
        Embedding::try_new(values).unwrap()
    }

    fn create_test_chunk(id: &str, content: &str, embedding: Embedding) -> IndexedChunk {
        IndexedChunk::builder()
            .chunk(
                Chunk::builder()
                    .id(ChunkId::new(uuid::Uuid::new_v4()))
                    .source(
                        ChunkSource::builder()
                            .file_path(
                                ForestRelativePath::try_new(format!("test/{}.md", id)).unwrap(),
                            )
                            .repo_name(RepoName::try_new("test").unwrap())
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
                            .text(content)
                            .token_count(TokenCount::try_new(10).unwrap())
                            .build(),
                    )
                    .context(ChunkContext::Markdown(
                        MarkdownContext::builder().heading_hierarchy(vec![]).build(),
                    ))
                    .build(),
            )
            .index_data(IndexData::Best { embedding })
            .indexed_at(Timestamp::now())
            .build()
    }

    #[test]
    fn test_semantic_engine_mode_returns_best() {
        // Given A semantic search engine
        let mock_repo = MockChunkRepository::new();
        let mock_provider = MockEmbeddingProvider::new();
        let engine = SemanticSearchEngine::new(mock_repo, Box::new(mock_provider));

        // When Getting the mode
        let mode = engine.mode();

        // Then It should return Best
        assert_eq!(mode, IndexMode::Best);
    }

    #[test]
    fn test_semantic_search_generates_query_embedding() {
        // Given A semantic search engine with embedding provider
        let mut mock_repo = MockChunkRepository::new();
        mock_repo
            .expect_search_semantic()
            .returning(|_, _| Ok(vec![]));

        let mut mock_provider = MockEmbeddingProvider::new();
        mock_provider
            .expect_embed()
            .times(1)
            .returning(|_| Ok(create_test_embedding(vec![0.1; 384])));

        let engine = SemanticSearchEngine::new(mock_repo, Box::new(mock_provider));
        let query = SearchQuery::builder()
            .text(QueryText::try_new("test query").unwrap())
            .mode(IndexMode::Best)
            .limit(ResultLimit::try_new(10).unwrap())
            .build();

        // When Searching
        let result = engine.search(&query);

        // Then Query embedding should be generated
        assert!(result.is_ok());
    }

    #[test]
    fn test_semantic_search_with_no_results() {
        // Given A repository with no matching chunks
        let mut mock_repo = MockChunkRepository::new();
        mock_repo
            .expect_search_semantic()
            .returning(|_, _| Ok(vec![]));

        let mut mock_provider = MockEmbeddingProvider::new();
        mock_provider
            .expect_embed()
            .returning(|_| Ok(create_test_embedding(vec![0.1; 384])));

        let engine = SemanticSearchEngine::new(mock_repo, Box::new(mock_provider));
        let query = SearchQuery::builder()
            .text(QueryText::try_new("nonexistent concept").unwrap())
            .mode(IndexMode::Best)
            .limit(ResultLimit::try_new(10).unwrap())
            .build();

        // When Searching
        let results = engine.search(&query).unwrap();

        // Then Results should be empty
        assert_eq!(results.results.len(), 0);
        assert_eq!(results.total_chunks_searched, 0);
    }

    #[test]
    fn test_semantic_search_returns_ranked_results() {
        // Given A repository with matching chunks
        let chunk1 = create_test_chunk(
            "1",
            "rust programming",
            create_test_embedding(vec![0.5; 384]),
        );
        let chunk2 = create_test_chunk("2", "rust language", create_test_embedding(vec![0.9; 384]));

        let mut mock_repo = MockChunkRepository::new();
        mock_repo
            .expect_search_semantic()
            .returning(move |_, _| Ok(vec![(chunk2.clone(), 0.95), (chunk1.clone(), 0.75)]));

        let mut mock_provider = MockEmbeddingProvider::new();
        mock_provider
            .expect_embed()
            .returning(|_| Ok(create_test_embedding(vec![0.8; 384])));

        let engine = SemanticSearchEngine::new(mock_repo, Box::new(mock_provider));
        let query = SearchQuery::builder()
            .text(QueryText::try_new("rust").unwrap())
            .mode(IndexMode::Best)
            .limit(ResultLimit::try_new(10).unwrap())
            .build();

        // When Searching
        let results = engine.search(&query).unwrap();

        // Then Results should be ranked by similarity score
        assert_eq!(results.results.len(), 2);
        assert!(results.results[0].score.into_inner() > results.results[1].score.into_inner());
        assert_eq!(results.total_chunks_searched, 2);
    }

    #[test]
    fn test_semantic_search_respects_limit() {
        // Given A repository with many matching chunks
        let chunk1 = create_test_chunk("1", "content 1", create_test_embedding(vec![0.1; 384]));
        let chunk2 = create_test_chunk("2", "content 2", create_test_embedding(vec![0.2; 384]));
        let chunk3 = create_test_chunk("3", "content 3", create_test_embedding(vec![0.3; 384]));

        let mut mock_repo = MockChunkRepository::new();
        mock_repo
            .expect_search_semantic()
            .returning(move |_, limit| {
                let all_results = vec![
                    (chunk1.clone(), 0.9),
                    (chunk2.clone(), 0.8),
                    (chunk3.clone(), 0.7),
                ];
                Ok(all_results.into_iter().take(limit).collect())
            });

        let mut mock_provider = MockEmbeddingProvider::new();
        mock_provider
            .expect_embed()
            .returning(|_| Ok(create_test_embedding(vec![0.5; 384])));

        let engine = SemanticSearchEngine::new(mock_repo, Box::new(mock_provider));
        let query = SearchQuery::builder()
            .text(QueryText::try_new("test").unwrap())
            .mode(IndexMode::Best)
            .limit(ResultLimit::try_new(2).unwrap())
            .build();

        // When Searching with limit of 2
        let results = engine.search(&query).unwrap();

        // Then Only 2 results should be returned
        assert_eq!(results.results.len(), 2);
    }

    #[test]
    fn test_semantic_search_rejects_fast_mode_query() {
        // Given A semantic engine (Best mode)
        let mock_repo = MockChunkRepository::new();
        let mock_provider = MockEmbeddingProvider::new();
        let engine = SemanticSearchEngine::new(mock_repo, Box::new(mock_provider));

        let query = SearchQuery::builder()
            .text(QueryText::try_new("test").unwrap())
            .mode(IndexMode::Fast)
            .limit(ResultLimit::try_new(10).unwrap())
            .build();

        // When Searching with Fast mode query
        let result = engine.search(&query);

        // Then It should return ModeMismatch error
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SearchError::ModeMismatch { .. }
        ));
    }

    #[test]
    fn test_semantic_search_propagates_embedding_errors() {
        // Given An embedding provider that returns an error
        let mock_repo = MockChunkRepository::new();
        let mut mock_provider = MockEmbeddingProvider::new();
        mock_provider.expect_embed().returning(|_| {
            Err(
                crate::knowledge::search::EmbeddingError::EmbeddingGenerationFailed {
                    source: Box::new(std::io::Error::other("test error")),
                },
            )
        });

        let engine = SemanticSearchEngine::new(mock_repo, Box::new(mock_provider));
        let query = SearchQuery::builder()
            .text(QueryText::try_new("test").unwrap())
            .mode(IndexMode::Best)
            .limit(ResultLimit::try_new(10).unwrap())
            .build();

        // When Searching
        let result = engine.search(&query);

        // Then Embedding error should be propagated
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SearchError::EmbeddingFailed { .. }
        ));
    }

    #[test]
    fn test_semantic_search_propagates_storage_errors() {
        // Given A repository that returns an error
        let mut mock_repo = MockChunkRepository::new();
        mock_repo.expect_search_semantic().returning(|_, _| {
            Err(crate::knowledge::storage::StorageError::InvalidData {
                message: "test error".to_string(),
            })
        });

        let mut mock_provider = MockEmbeddingProvider::new();
        mock_provider
            .expect_embed()
            .returning(|_| Ok(create_test_embedding(vec![0.1; 384])));

        let engine = SemanticSearchEngine::new(mock_repo, Box::new(mock_provider));
        let query = SearchQuery::builder()
            .text(QueryText::try_new("test").unwrap())
            .mode(IndexMode::Best)
            .limit(ResultLimit::try_new(10).unwrap())
            .build();

        // When Searching
        let result = engine.search(&query);

        // Then Storage error should be propagated
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SearchError::Storage { .. }));
    }

    #[test]
    fn test_semantic_search_passes_embedding_to_repository() {
        // Given A semantic search engine
        let query_embedding = vec![0.42; 384];
        let expected_embedding = query_embedding.clone();

        let mut mock_repo = MockChunkRepository::new();
        mock_repo
            .expect_search_semantic()
            .withf(move |embedding, _| embedding == expected_embedding.as_slice())
            .returning(|_, _| Ok(vec![]));

        let mut mock_provider = MockEmbeddingProvider::new();
        mock_provider
            .expect_embed()
            .returning(move |_| Ok(create_test_embedding(query_embedding.clone())));

        let engine = SemanticSearchEngine::new(mock_repo, Box::new(mock_provider));
        let query = SearchQuery::builder()
            .text(QueryText::try_new("test").unwrap())
            .mode(IndexMode::Best)
            .limit(ResultLimit::try_new(10).unwrap())
            .build();

        // When Searching
        let result = engine.search(&query);

        // Then Query embedding should be passed to repository
        assert!(result.is_ok());
    }

    #[test_case("semantic search" ; "multi-word query")]
    #[test_case("rust" ; "single word")]
    #[test_case("How does Bottlerocket boot?" ; "question")]
    fn test_semantic_search_handles_various_queries(query_text: &str) {
        // Given A semantic search engine
        let chunk = create_test_chunk("1", "test content", create_test_embedding(vec![0.5; 384]));

        let mut mock_repo = MockChunkRepository::new();
        mock_repo
            .expect_search_semantic()
            .returning(move |_, _| Ok(vec![(chunk.clone(), 0.8)]));

        let mut mock_provider = MockEmbeddingProvider::new();
        mock_provider
            .expect_embed()
            .returning(|_| Ok(create_test_embedding(vec![0.6; 384])));

        let engine = SemanticSearchEngine::new(mock_repo, Box::new(mock_provider));
        let query = SearchQuery::builder()
            .text(QueryText::try_new(query_text).unwrap())
            .mode(IndexMode::Best)
            .limit(ResultLimit::try_new(10).unwrap())
            .build();

        // When Searching
        let result = engine.search(&query);

        // Then Search should succeed
        assert!(result.is_ok());
    }

    #[test]
    fn test_semantic_search_includes_empty_matched_terms() {
        // Given A repository with matching chunks
        let chunk = create_test_chunk("1", "rust content", create_test_embedding(vec![0.5; 384]));

        let mut mock_repo = MockChunkRepository::new();
        mock_repo
            .expect_search_semantic()
            .returning(move |_, _| Ok(vec![(chunk.clone(), 0.85)]));

        let mut mock_provider = MockEmbeddingProvider::new();
        mock_provider
            .expect_embed()
            .returning(|_| Ok(create_test_embedding(vec![0.6; 384])));

        let engine = SemanticSearchEngine::new(mock_repo, Box::new(mock_provider));
        let query = SearchQuery::builder()
            .text(QueryText::try_new("rust").unwrap())
            .mode(IndexMode::Best)
            .limit(ResultLimit::try_new(10).unwrap())
            .build();

        // When Searching
        let results = engine.search(&query).unwrap();

        // Then Matched terms should be empty for semantic search
        assert_eq!(results.results.len(), 1);
        assert!(results.results[0].matched_terms.is_empty());
    }
}
