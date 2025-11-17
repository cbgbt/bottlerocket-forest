//! BM25 keyword search implementation
//!
//! Implements the SearchEngine trait using BM25 (Best Matching 25) algorithm
//! for keyword-based search. BM25 ranks documents based on term frequency
//! and inverse document frequency.

use crate::knowledge::domain::{
    IndexMode, MatchedTerm, RelevanceScore, SearchQuery, SearchResult, SearchResults,
};
use crate::knowledge::storage::ChunkRepository;
use snafu::ResultExt;
use std::time::Instant;

use super::{SearchEngine, SearchError};

/// BM25 search engine for Fast mode keyword search
pub struct Bm25SearchEngine<R: ChunkRepository> {
    repository: R,
}

impl<R: ChunkRepository> Bm25SearchEngine<R> {
    /// Create a new BM25 search engine with the given repository
    pub fn new(repository: R) -> Self {
        Self { repository }
    }
}

impl<R: ChunkRepository> SearchEngine for Bm25SearchEngine<R> {
    fn search(&self, query: &SearchQuery) -> Result<SearchResults, SearchError> {
        use super::engine::search_error::*;

        let start = Instant::now();

        snafu::ensure!(
            query.mode == IndexMode::Fast,
            ModeMismatchSnafu {
                query_mode: query.mode,
                engine_mode: IndexMode::Fast,
            }
        );

        let query_terms: Vec<String> = query
            .text
            .to_string()
            .split_whitespace()
            .map(|s| s.to_lowercase())
            .collect();

        let raw_results = self
            .repository
            .search_bm25(&query_terms, query.limit.into_inner())
            .context(StorageSnafu)?;

        let results = raw_results
            .into_iter()
            .map(|(indexed_chunk, score)| {
                let matched_terms: Vec<MatchedTerm> = query_terms
                    .iter()
                    .map(|t| MatchedTerm::try_new(t.clone()).expect("non-empty term"))
                    .collect();

                SearchResult::builder()
                    .chunk(indexed_chunk.chunk)
                    .score(RelevanceScore::try_new(score).expect("score in valid range"))
                    .matched_terms(matched_terms)
                    .build()
            })
            .collect::<Vec<_>>();

        let total_chunks_searched = results.len();

        Ok(SearchResults::builder()
            .query(query.clone())
            .results(results)
            .total_chunks_searched(total_chunks_searched)
            .search_duration(start.elapsed())
            .build())
    }

    fn mode(&self) -> IndexMode {
        IndexMode::Fast
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::knowledge::domain::{
        Chunk, ChunkContent, ChunkContext, ChunkId, ChunkSource, ForestRelativePath, IndexData,
        IndexedChunk, LineCount, LineNumber, LineRange, MarkdownContext, QueryText, RepoName,
        ResultLimit, Timestamp, TokenCount,
    };
    use crate::knowledge::storage::repository::MockChunkRepository;
    use std::collections::BTreeMap;
    use test_case::test_case;

    fn create_test_chunk(id: &str, content: &str) -> IndexedChunk {
        let mut bm25_terms = BTreeMap::new();
        for term in content.split_whitespace() {
            *bm25_terms.entry(term.to_lowercase()).or_insert(0) += 1;
        }

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
            .index_data(IndexData::Fast { bm25_terms })
            .indexed_at(Timestamp::now())
            .build()
    }

    #[test]
    fn test_bm25_engine_mode_returns_fast() {
        // Given A BM25 search engine
        let mock_repo = MockChunkRepository::new();
        let engine = Bm25SearchEngine::new(mock_repo);

        // When Getting the mode
        let mode = engine.mode();

        // Then It should return Fast
        assert_eq!(mode, IndexMode::Fast);
    }

    #[test]
    fn test_bm25_search_with_no_results() {
        // Given A repository with no matching chunks
        let mut mock_repo = MockChunkRepository::new();
        mock_repo.expect_search_bm25().returning(|_, _| Ok(vec![]));

        let engine = Bm25SearchEngine::new(mock_repo);
        let query = SearchQuery::builder()
            .text(QueryText::try_new("nonexistent").unwrap())
            .mode(IndexMode::Fast)
            .limit(ResultLimit::try_new(10).unwrap())
            .build();

        // When Searching
        let results = engine.search(&query).unwrap();

        // Then Results should be empty
        assert_eq!(results.results.len(), 0);
        assert_eq!(results.total_chunks_searched, 0);
    }

    #[test]
    fn test_bm25_search_returns_ranked_results() {
        // Given A repository with matching chunks
        let chunk1 = create_test_chunk("1", "rust programming language");
        let chunk2 = create_test_chunk("2", "rust rust rust");

        let mut mock_repo = MockChunkRepository::new();
        mock_repo
            .expect_search_bm25()
            .returning(move |_, _| Ok(vec![(chunk2.clone(), 0.9), (chunk1.clone(), 0.5)]));

        let engine = Bm25SearchEngine::new(mock_repo);
        let query = SearchQuery::builder()
            .text(QueryText::try_new("rust").unwrap())
            .mode(IndexMode::Fast)
            .limit(ResultLimit::try_new(10).unwrap())
            .build();

        // When Searching
        let results = engine.search(&query).unwrap();

        // Then Results should be ranked by score
        assert_eq!(results.results.len(), 2);
        assert!(results.results[0].score.into_inner() > results.results[1].score.into_inner());
        assert_eq!(results.total_chunks_searched, 2);
    }

    #[test]
    fn test_bm25_search_respects_limit() {
        // Given A repository with many matching chunks
        let chunk1 = create_test_chunk("1", "rust");
        let chunk2 = create_test_chunk("2", "rust");
        let chunk3 = create_test_chunk("3", "rust");

        let mut mock_repo = MockChunkRepository::new();
        mock_repo.expect_search_bm25().returning(move |_, limit| {
            let all_results = vec![
                (chunk1.clone(), 0.9),
                (chunk2.clone(), 0.8),
                (chunk3.clone(), 0.7),
            ];
            Ok(all_results.into_iter().take(limit).collect())
        });

        let engine = Bm25SearchEngine::new(mock_repo);
        let query = SearchQuery::builder()
            .text(QueryText::try_new("rust").unwrap())
            .mode(IndexMode::Fast)
            .limit(ResultLimit::try_new(2).unwrap())
            .build();

        // When Searching with limit of 2
        let results = engine.search(&query).unwrap();

        // Then Only 2 results should be returned
        assert_eq!(results.results.len(), 2);
    }

    #[test]
    fn test_bm25_search_rejects_best_mode_query() {
        // Given A BM25 engine (Fast mode)
        let mock_repo = MockChunkRepository::new();
        let engine = Bm25SearchEngine::new(mock_repo);

        let query = SearchQuery::builder()
            .text(QueryText::try_new("test").unwrap())
            .mode(IndexMode::Best)
            .limit(ResultLimit::try_new(10).unwrap())
            .build();

        // When Searching with Best mode query
        let result = engine.search(&query);

        // Then It should return ModeMismatch error
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SearchError::ModeMismatch { .. }
        ));
    }

    #[test]
    fn test_bm25_search_extracts_matched_terms() {
        // Given A repository with matching chunks
        let chunk = create_test_chunk("1", "rust programming language");

        let mut mock_repo = MockChunkRepository::new();
        mock_repo
            .expect_search_bm25()
            .returning(move |_, _| Ok(vec![(chunk.clone(), 0.8)]));

        let engine = Bm25SearchEngine::new(mock_repo);
        let query = SearchQuery::builder()
            .text(QueryText::try_new("rust programming").unwrap())
            .mode(IndexMode::Fast)
            .limit(ResultLimit::try_new(10).unwrap())
            .build();

        // When Searching
        let results = engine.search(&query).unwrap();

        // Then Matched terms should be extracted
        assert_eq!(results.results.len(), 1);
        assert!(!results.results[0].matched_terms.is_empty());
    }

    #[test_case("rust" ; "single term")]
    #[test_case("rust programming" ; "multiple terms")]
    #[test_case("Rust Programming Language" ; "mixed case")]
    fn test_bm25_search_handles_various_queries(query_text: &str) {
        // Given A repository with chunks
        let chunk = create_test_chunk("1", "rust programming language");

        let mut mock_repo = MockChunkRepository::new();
        mock_repo
            .expect_search_bm25()
            .returning(move |_, _| Ok(vec![(chunk.clone(), 0.7)]));

        let engine = Bm25SearchEngine::new(mock_repo);
        let query = SearchQuery::builder()
            .text(QueryText::try_new(query_text).unwrap())
            .mode(IndexMode::Fast)
            .limit(ResultLimit::try_new(10).unwrap())
            .build();

        // When Searching
        let results = engine.search(&query);

        // Then Search should succeed
        assert!(results.is_ok());
    }

    #[test]
    fn test_bm25_search_propagates_storage_errors() {
        // Given A repository that returns an error
        let mut mock_repo = MockChunkRepository::new();
        mock_repo.expect_search_bm25().returning(|_, _| {
            Err(crate::knowledge::storage::StorageError::InvalidData {
                message: "test error".to_string(),
            })
        });

        let engine = Bm25SearchEngine::new(mock_repo);
        let query = SearchQuery::builder()
            .text(QueryText::try_new("test").unwrap())
            .mode(IndexMode::Fast)
            .limit(ResultLimit::try_new(10).unwrap())
            .build();

        // When Searching
        let result = engine.search(&query);

        // Then Storage error should be propagated
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SearchError::Storage { .. }));
    }
}
