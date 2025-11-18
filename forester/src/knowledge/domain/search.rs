//! Search domain types
//!
//! Types for representing search queries and results.
//!
//! The search flow:
//! * [`SearchQuery`] specifies what to search for (text, mode, limit)
//! * [`SearchResult`] represents a single matched chunk with score and matched terms
//! * [`SearchResults`] aggregates all results with metadata about the search operation

use bon::Builder;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::{Chunk, IndexMode, MatchedTerm, QueryText, RelevanceScore, ResultLimit};

/// A search query with parameters
#[derive(Debug, Clone, PartialEq, Eq, Builder, Serialize, Deserialize)]
#[builder(on(_, into))]
#[non_exhaustive]
pub struct SearchQuery {
    pub text: QueryText,
    pub mode: IndexMode,
    pub limit: ResultLimit,
}

/// A single search result
#[derive(Debug, Clone, PartialEq, Builder, Serialize, Deserialize)]
#[builder(on(_, into))]
#[non_exhaustive]
pub struct SearchResult {
    pub chunk: Chunk,
    pub score: RelevanceScore,
    pub matched_terms: Vec<MatchedTerm>,
}

/// Results from a search operation
#[derive(Debug, Clone, PartialEq, Builder, Serialize, Deserialize)]
#[builder(on(_, into))]
#[non_exhaustive]
pub struct SearchResults {
    pub query: SearchQuery,
    pub results: Vec<SearchResult>,
    pub total_chunks_searched: usize,
    pub search_duration: Duration,
}
