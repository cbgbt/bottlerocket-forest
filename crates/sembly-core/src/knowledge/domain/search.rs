//! Search domain types
//!
//! Search flow:
//! * [`SearchQuery`] specifies search parameters
//! * [`SearchResult`] represents a single matched chunk with relevance score
//! * [`SearchResults`] aggregates all results with search metadata
//! * [`FileSearchResult`] groups chunk matches by source file

use bon::Builder;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::{
    Chunk, ContextId, ForestRelativePath, QueryText, RelevanceScore, RepoName, ResultLimit,
};

/// Search parameters combining query text and result limit
#[derive(Debug, Clone, PartialEq, Eq, Builder, Serialize, Deserialize)]
#[builder(on(_, into))]
#[non_exhaustive]
pub struct SearchQuery {
    pub text: QueryText,
    pub limit: ResultLimit,
    /// Optional context to scope search results; None searches all contexts
    pub context_id: Option<ContextId>,
}

/// Single chunk match with relevance score
#[derive(Debug, Clone, PartialEq, Builder, Serialize, Deserialize)]
#[builder(on(_, into))]
#[non_exhaustive]
pub struct SearchResult {
    pub chunk: Chunk,
    pub score: RelevanceScore,
}

/// Complete search operation results with performance metrics
#[derive(Debug, Clone, PartialEq, Builder, Serialize, Deserialize)]
#[builder(on(_, into))]
#[non_exhaustive]
pub struct SearchResults {
    pub query: SearchQuery,
    pub results: Vec<SearchResult>,
    pub total_chunks_searched: usize,
    pub search_duration: Duration,
}

/// Chunk matches grouped by source file
#[derive(Debug, Clone, PartialEq, Builder, Serialize, Deserialize)]
#[builder(on(_, into))]
#[non_exhaustive]
pub struct FileSearchResult {
    pub file_path: ForestRelativePath,
    pub repo_name: RepoName,
    pub match_count: usize,
    pub best_score: RelevanceScore,
    pub chunks: Vec<SearchResult>,
}
