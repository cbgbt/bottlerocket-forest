//! Semantic search implementation using embeddings
//!
//! Implements the SearchEngine trait using semantic similarity search with
//! embeddings. This engine generates embeddings for query text and finds
//! chunks with similar semantic meaning using vector similarity.
//!
//! For embedding generation, see [`crate::knowledge::search::embeddings`].
//! For vector similarity search implementation, see [`crate::knowledge::storage::sqlite::search::search_semantic`].

use snafu::ResultExt;

use crate::knowledge::domain::{SearchQuery, SearchResults};
use crate::knowledge::scoring::ScoreBooster;
use crate::knowledge::storage::ChunkRepository;

use super::{EmbeddingProvider, SearchEngine, SearchError};

/// Semantic search engine using embedding-based similarity
///
/// Converts query text into semantic vectors and searches for chunks with similar
/// embeddings using cosine similarity. Applies score boosting based on file characteristics.
pub struct SemanticSearchEngine<R: ChunkRepository> {
    repository: R,
    embedding_provider: Box<dyn EmbeddingProvider>,
    score_booster: ScoreBooster,
}

impl<R: ChunkRepository> SemanticSearchEngine<R> {
    /// Create a semantic search engine
    pub fn new(
        repository: R,
        embedding_provider: Box<dyn EmbeddingProvider>,
        score_booster: ScoreBooster,
    ) -> Self {
        Self {
            repository,
            embedding_provider,
            score_booster,
        }
    }
}

impl<R: ChunkRepository> SearchEngine for SemanticSearchEngine<R> {
    fn search(&self, query: &SearchQuery) -> Result<SearchResults, SearchError> {
        use super::engine::search_error::*;
        use std::time::Instant;

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

        let mut search_results: Vec<_> = results
            .into_iter()
            .map(|(indexed_chunk, score)| {
                use crate::knowledge::domain::{RelevanceScore, SearchResult};
                // Normalize cosine similarity from [-1, 1] to [0, 1]
                let normalized_score = (score + 1.0) / 2.0;
                let relevance_score = RelevanceScore::try_new(normalized_score)
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                    .context(InvalidScoreSnafu {
                        score: normalized_score,
                    })?;

                // Apply score boosting based on file characteristics
                let boosted_score = self
                    .score_booster
                    .apply_boost(relevance_score, &indexed_chunk.chunk)
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                    .context(InvalidScoreSnafu {
                        score: relevance_score.into_inner(),
                    })?;

                Ok(SearchResult::builder()
                    .chunk(indexed_chunk.chunk)
                    .score(boosted_score)
                    .build())
            })
            .collect::<Result<Vec<_>, SearchError>>()?;

        // Sort by boosted score (descending)
        search_results.sort_by_key(|result| std::cmp::Reverse(result.score));

        Ok(SearchResults::builder()
            .query(query.clone())
            .results(search_results)
            .total_chunks_searched(total_chunks_searched)
            .search_duration(search_duration)
            .build())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::knowledge::domain::{
        Chunk, ChunkContent, ChunkContext, ChunkId, ChunkSource, Embedding, ForestRelativePath,
        IndexedChunk, MarkdownContext, QueryText, RepoName, ResultLimit, Timestamp, TokenCount,
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
            .embedding(embedding)
            .indexed_at(Timestamp::now())
            .build()
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

        let engine =
            SemanticSearchEngine::new(mock_repo, Box::new(mock_provider), ScoreBooster::default());
        let query = SearchQuery::builder()
            .text(QueryText::try_new("test query").unwrap())
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

        let engine =
            SemanticSearchEngine::new(mock_repo, Box::new(mock_provider), ScoreBooster::default());
        let query = SearchQuery::builder()
            .text(QueryText::try_new("nonexistent concept").unwrap())
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

        // Use empty boost rules to test raw score ordering
        let score_booster = crate::knowledge::scoring::ScoreBooster::new(vec![]);
        let engine = SemanticSearchEngine {
            repository: mock_repo,
            embedding_provider: Box::new(mock_provider),
            score_booster,
        };
        let query = SearchQuery::builder()
            .text(QueryText::try_new("rust").unwrap())
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

        let engine =
            SemanticSearchEngine::new(mock_repo, Box::new(mock_provider), ScoreBooster::default());
        let query = SearchQuery::builder()
            .text(QueryText::try_new("test").unwrap())
            .limit(ResultLimit::try_new(2).unwrap())
            .build();

        // When Searching with limit of 2
        let results = engine.search(&query).unwrap();

        // Then Only 2 results should be returned
        assert_eq!(results.results.len(), 2);
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

        let engine =
            SemanticSearchEngine::new(mock_repo, Box::new(mock_provider), ScoreBooster::default());
        let query = SearchQuery::builder()
            .text(QueryText::try_new("test").unwrap())
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

        let engine =
            SemanticSearchEngine::new(mock_repo, Box::new(mock_provider), ScoreBooster::default());
        let query = SearchQuery::builder()
            .text(QueryText::try_new("test").unwrap())
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

        let engine =
            SemanticSearchEngine::new(mock_repo, Box::new(mock_provider), ScoreBooster::default());
        let query = SearchQuery::builder()
            .text(QueryText::try_new("test").unwrap())
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

        let engine =
            SemanticSearchEngine::new(mock_repo, Box::new(mock_provider), ScoreBooster::default());
        let query = SearchQuery::builder()
            .text(QueryText::try_new(query_text).unwrap())
            .limit(ResultLimit::try_new(10).unwrap())
            .build();

        // When Searching
        let result = engine.search(&query);

        // Then Search should succeed
        assert!(result.is_ok());
    }

    #[test]
    fn test_semantic_search_returns_results() {
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

        let engine =
            SemanticSearchEngine::new(mock_repo, Box::new(mock_provider), ScoreBooster::default());
        let query = SearchQuery::builder()
            .text(QueryText::try_new("rust").unwrap())
            .limit(ResultLimit::try_new(10).unwrap())
            .build();

        // When Searching
        let results = engine.search(&query).unwrap();

        // Then Results should be returned
        assert_eq!(results.results.len(), 1);
    }

    #[test]
    fn test_semantic_search_reorders_after_boosting() {
        // Given Two chunks where boosting will reverse their order
        // chunk1: .rs file with higher raw score (0.9)
        // chunk2: .md file with lower raw score (0.7) but gets 1.5x boost
        let chunk1 = IndexedChunk::builder()
            .chunk(
                Chunk::builder()
                    .id(ChunkId::new(uuid::Uuid::new_v4()))
                    .source(
                        ChunkSource::builder()
                            .file_path(ForestRelativePath::try_new("src/main.rs").unwrap())
                            .repo_name(RepoName::try_new("test").unwrap())
                            .build(),
                    )
                    .content(
                        ChunkContent::builder()
                            .text("rust code")
                            .token_count(TokenCount::try_new(10).unwrap())
                            .build(),
                    )
                    .context(ChunkContext::Markdown(
                        MarkdownContext::builder().heading_hierarchy(vec![]).build(),
                    ))
                    .build(),
            )
            .embedding(create_test_embedding(vec![0.9; 384]))
            .indexed_at(Timestamp::now())
            .build();

        let chunk2 = IndexedChunk::builder()
            .chunk(
                Chunk::builder()
                    .id(ChunkId::new(uuid::Uuid::new_v4()))
                    .source(
                        ChunkSource::builder()
                            .file_path(ForestRelativePath::try_new("docs/guide.md").unwrap())
                            .repo_name(RepoName::try_new("test").unwrap())
                            .build(),
                    )
                    .content(
                        ChunkContent::builder()
                            .text("documentation")
                            .token_count(TokenCount::try_new(10).unwrap())
                            .build(),
                    )
                    .context(ChunkContext::Markdown(
                        MarkdownContext::builder().heading_hierarchy(vec![]).build(),
                    ))
                    .build(),
            )
            .embedding(create_test_embedding(vec![0.7; 384]))
            .indexed_at(Timestamp::now())
            .build();

        let mut mock_repo = MockChunkRepository::new();
        mock_repo.expect_search_semantic().returning(move |_, _| {
            // Repository returns in raw score order (chunk1 first)
            Ok(vec![(chunk1.clone(), 0.9), (chunk2.clone(), 0.7)])
        });

        let mut mock_provider = MockEmbeddingProvider::new();
        mock_provider
            .expect_embed()
            .returning(|_| Ok(create_test_embedding(vec![0.8; 384])));

        // Boost markdown files by 1.5x
        let boost_rules = vec![
            crate::knowledge::scoring::BoostRule::builder()
                .description("Markdown files")
                .pattern(crate::knowledge::scoring::BoostPattern::new("**/*.md").unwrap())
                .multiplier(crate::knowledge::scoring::BoostMultiplier::try_new(1.5).unwrap())
                .build(),
        ];
        let score_booster = crate::knowledge::scoring::ScoreBooster::new(boost_rules);
        let engine = SemanticSearchEngine {
            repository: mock_repo,
            embedding_provider: Box::new(mock_provider),
            score_booster,
        };

        let query = SearchQuery::builder()
            .text(QueryText::try_new("test").unwrap())
            .limit(ResultLimit::try_new(10).unwrap())
            .build();

        // When Searching
        let results = engine.search(&query).unwrap();

        // Then Results should be reordered by boosted score
        // chunk2 (.md): 0.85 * 1.5 = 1.0 (clamped)
        // chunk1 (.rs): 0.95 * 1.0 = 0.95
        assert_eq!(results.results.len(), 2);
        assert!(
            results.results[0]
                .chunk
                .source
                .file_path
                .to_string()
                .ends_with(".md")
        );
        assert!(
            results.results[1]
                .chunk
                .source
                .file_path
                .to_string()
                .ends_with(".rs")
        );
        assert_eq!(results.results[0].score.into_inner(), 1.0);
        assert_eq!(results.results[1].score.into_inner(), 0.95);
    }
}
