//! Domain types for knowledge indexing
//!
//! Core abstractions:
//! * [`Chunk`] and related types represent searchable documentation units
//! * [`SearchQuery`] and [`SearchResults`] handle search operations
//! * [`FileType`] classifies files for indexing
//! * Newtypes provide type-safe wrappers for domain concepts

pub mod chunk;
pub mod file_type;
pub mod search;

use bon::Builder;
use nutype::nutype;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

use crate::knowledge::constants;

pub use chunk::{
    Chunk, ChunkContent, ChunkContext, ChunkSource, MarkdownContext, RustDocContext, Visibility,
};
pub use file_type::FileType;
pub use search::{FileSearchResult, SearchQuery, SearchResult, SearchResults};

/// Controls which files are scanned during indexing
#[derive(Debug, Clone, Builder)]
#[builder(on(_, into))]
#[non_exhaustive]
pub struct ScanConfig {
    #[builder(default = true)]
    pub respect_gitignore: bool,

    #[builder(default = true)]
    pub use_foresterignore: bool,

    /// Empty means scan entire forest root; non-empty restricts to specified paths
    #[builder(default)]
    pub targets: Vec<PathBuf>,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            respect_gitignore: true,
            use_foresterignore: true,
            targets: Vec::new(),
        }
    }
}

/// Unique identifier for a documentation chunk
#[nutype(derive(Debug, Clone, Copy, Display, Serialize, Deserialize, PartialEq, Eq))]
pub struct ChunkId(Uuid);

/// Repository name within the forest
#[nutype(
    validate(not_empty),
    derive(Debug, Clone, Display, Serialize, Deserialize, PartialEq, Eq)
)]
pub struct RepoName(String);

/// Name of a Rust item extracted from source
#[nutype(
    validate(not_empty),
    derive(Debug, Clone, Display, Serialize, Deserialize, PartialEq, Eq)
)]
pub struct ItemName(String);

/// Markdown heading text for building document hierarchies
#[nutype(
    validate(not_empty),
    derive(Debug, Clone, Display, Serialize, Deserialize, PartialEq, Eq)
)]
pub struct HeadingText(String);

/// Search query text for semantic or keyword matching
#[nutype(
    validate(not_empty),
    derive(Debug, Clone, Display, Serialize, Deserialize, PartialEq, Eq)
)]
pub struct QueryText(String);

/// File path relative to forest root for portable indexing
#[nutype(
    validate(not_empty),
    derive(Debug, Clone, Display, Serialize, Deserialize, PartialEq, Eq, Hash)
)]
pub struct ForestRelativePath(String);

/// Absolute filesystem path for file operations
#[nutype(
    validate(not_empty),
    derive(Debug, Clone, Display, Serialize, Deserialize, PartialEq, Eq)
)]
pub struct AbsolutePath(String);

/// Token count for chunk size enforcement
#[nutype(
    validate(greater = 0),
    derive(Debug, Clone, Copy, Display, Serialize, Deserialize, PartialEq, Eq)
)]
pub struct TokenCount(usize);

/// Maximum search results to return, bounded 1-100
#[nutype(
    validate(greater = 0, less_or_equal = 100),
    derive(Debug, Clone, Copy, Display, Serialize, Deserialize, PartialEq, Eq)
)]
pub struct ResultLimit(usize);

/// Search result relevance score normalized 0.0-1.0
#[nutype(
    validate(greater_or_equal = 0.0, less_or_equal = 1.0),
    derive(Debug, Clone, Copy, Display, Serialize, Deserialize, PartialEq)
)]
pub struct RelevanceScore(f32);

impl Eq for RelevanceScore {}

impl PartialOrd for RelevanceScore {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RelevanceScore {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // SAFETY: RelevanceScore is validated to be in [0.0, 1.0], so it cannot be NaN.
        // Therefore f32::partial_cmp always returns Some and unwrap is safe.
        self.into_inner()
            .partial_cmp(&other.into_inner())
            .expect("RelevanceScore is validated to exclude NaN")
    }
}

/// Rust item signature for doc comment context
#[nutype(
    validate(not_empty),
    derive(Debug, Clone, Display, Serialize, Deserialize, PartialEq, Eq)
)]
pub struct Signature(String);

/// Semantic embedding vector for similarity search
#[nutype(
    validate(predicate = |v: &Vec<f32>| !v.is_empty() && !is_zero_vector(v)),
    derive(Debug, Clone, Serialize, Deserialize, PartialEq, AsRef, Deref)
)]
pub struct Embedding(Vec<f32>);

fn is_zero_vector(v: &[f32]) -> bool {
    v.iter().all(|&x| x == 0.0)
}

/// Raw text content ready for chunking
#[nutype(derive(Debug, Clone, Display, AsRef, Serialize, Deserialize, PartialEq, Eq))]
pub struct ChunkableContent(String);

/// Embedding model configuration that determines index structure
///
/// Changes to these parameters require rebuilding the entire index.
#[derive(Debug, Clone, PartialEq, Builder, Serialize, Deserialize)]
#[non_exhaustive]
pub struct EmbeddingModelConfig {
    #[builder(into)]
    pub model_name: String,
    pub embedding_dim: usize,
    pub max_tokens: usize,
    pub overlap_tokens: usize,
}

impl Default for EmbeddingModelConfig {
    fn default() -> Self {
        Self {
            model_name: constants::DEFAULT_TOKENIZER_MODEL.to_string(),
            embedding_dim: constants::EMBEDDING_DIM,
            max_tokens: constants::DEFAULT_MAX_CHUNK_TOKENS,
            overlap_tokens: constants::DEFAULT_CHUNK_OVERLAP_TOKENS,
        }
    }
}

/// Domain chunk with embedding and indexing timestamp
#[derive(Debug, Clone, Builder)]
#[non_exhaustive]
pub struct IndexedChunk {
    pub chunk: Chunk,
    pub embedding: Embedding,
    pub indexed_at: Timestamp,
}

/// Unix timestamp for staleness detection
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp(i64);

impl Timestamp {
    /// Creates a timestamp from Unix seconds
    pub fn from_secs(secs: i64) -> Self {
        Self(secs)
    }

    /// Returns the Unix seconds value
    pub fn as_secs(&self) -> i64 {
        self.0
    }

    /// Creates a timestamp for the current system time
    pub fn now() -> Self {
        Self(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time before Unix epoch")
                .as_secs() as i64,
        )
    }
}

/// Statistics and configuration snapshot of the index
#[derive(Debug, Clone, PartialEq, Builder, Serialize, Deserialize)]
#[non_exhaustive]
pub struct IndexMetadata {
    pub last_build: std::time::SystemTime,
    pub chunk_count: usize,
    pub file_count: usize,
    pub model_config: EmbeddingModelConfig,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_repo_name_rejects_empty() {
        // Given An empty string
        let empty = RepoName::try_new("");

        // When Creating the newtype
        // Then It should fail validation
        assert!(empty.is_err());
    }

    #[test]
    fn test_token_count_rejects_zero() {
        // Given A zero value
        let zero = TokenCount::try_new(0);

        // When Creating the newtype
        // Then It should fail validation
        assert!(zero.is_err());
    }

    #[test]
    fn test_result_limit_rejects_zero() {
        // Given A zero value
        let zero = ResultLimit::try_new(0);

        // When Creating the newtype
        // Then It should fail validation
        assert!(zero.is_err());
    }

    #[test]
    fn test_result_limit_rejects_over_100() {
        // Given A value over 100
        let too_large = ResultLimit::try_new(101);

        // When Creating the newtype
        // Then It should fail validation
        assert!(too_large.is_err());
    }

    #[test]
    fn test_relevance_score_rejects_negative() {
        // Given A negative value
        let negative = RelevanceScore::try_new(-0.1);

        // When Creating the newtype
        // Then It should fail validation
        assert!(negative.is_err());
    }

    #[test]
    fn test_relevance_score_rejects_over_one() {
        // Given A value over 1.0
        let too_large = RelevanceScore::try_new(1.1);

        // When Creating the newtype
        // Then It should fail validation
        assert!(too_large.is_err());
    }

    #[test]
    fn test_embedding_rejects_empty() {
        // Given An empty vector
        let empty = Embedding::try_new(vec![]);

        // When Creating the newtype
        // Then It should fail validation
        assert!(empty.is_err());
    }

    #[test]
    fn test_embedding_accepts_non_empty() {
        // Given A non-empty vector
        let valid = Embedding::try_new(vec![0.1, 0.2, 0.3]);

        // When Creating the newtype
        // Then It should succeed
        assert!(valid.is_ok());
    }
}
