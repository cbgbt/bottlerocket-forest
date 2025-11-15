//! Domain types for knowledge indexing
//!
//! This module defines the core types used throughout the knowledge indexing system.

pub mod chunk;
pub mod file_type;
pub mod index_mode;
pub mod search;

use nutype::nutype;
use uuid::Uuid;

pub use chunk::{
    Chunk, ChunkContent, ChunkContext, ChunkSource, IndexData, LineRange, MarkdownContext,
    RustDocContext, Visibility,
};
pub use file_type::FileType;
pub use index_mode::{IndexMode, InvalidIndexMode};
pub use search::{SearchQuery, SearchResult, SearchResults};

/// Unique identifier for a documentation chunk
///
/// Each chunk in the index has a unique UUID to enable efficient lookups
/// and prevent duplicates.
#[nutype(derive(Debug, Clone, Copy, Display, Serialize, Deserialize, PartialEq, Eq))]
pub struct ChunkId(Uuid);

/// Name of a repository in the forest
///
/// Used to identify which repository a chunk belongs to (e.g., "bottlerocket", "twoliter").
#[nutype(
    validate(not_empty),
    derive(Debug, Clone, Display, Serialize, Deserialize, PartialEq, Eq)
)]
pub struct RepoName(String);

/// Name of a Rust item (function, struct, module, etc.)
///
/// Extracted from Rust source files to provide context for doc comments.
#[nutype(
    validate(not_empty),
    derive(Debug, Clone, Display, Serialize, Deserialize, PartialEq, Eq)
)]
pub struct ItemName(String);

/// Text of a markdown heading
///
/// Used to build heading hierarchies for markdown chunks, providing context
/// about where in the document structure a chunk appears.
#[nutype(
    validate(not_empty),
    derive(Debug, Clone, Display, Serialize, Deserialize, PartialEq, Eq)
)]
pub struct HeadingText(String);

/// User's search query text
///
/// The text that will be used for semantic or keyword search against the index.
#[nutype(
    validate(not_empty),
    derive(Debug, Clone, Display, Serialize, Deserialize, PartialEq, Eq)
)]
pub struct QueryText(String);

/// Path relative to the forest root
///
/// All indexed files are stored with paths relative to the forest root directory,
/// making the index portable across different machines.
#[nutype(
    validate(not_empty),
    derive(Debug, Clone, Display, Serialize, Deserialize, PartialEq, Eq)
)]
pub struct ForestRelativePath(String);

/// Absolute filesystem path
///
/// Used for file operations during indexing and scanning.
#[nutype(
    validate(not_empty),
    derive(Debug, Clone, Display, Serialize, Deserialize, PartialEq, Eq)
)]
pub struct AbsolutePath(String);

/// Number of tokens in a text chunk
///
/// Used to enforce maximum chunk sizes and track index statistics.
/// Must be greater than 0.
#[nutype(
    validate(greater = 0),
    derive(Debug, Clone, Copy, Display, Serialize, Deserialize, PartialEq, Eq)
)]
pub struct TokenCount(usize);

/// Line number in a source file
///
/// Used to specify the location of chunks within files for precise navigation.
/// Must be greater than 0 (1-indexed).
#[nutype(
    validate(greater = 0),
    derive(Debug, Clone, Copy, Display, Serialize, Deserialize, PartialEq, Eq)
)]
pub struct LineNumber(usize);

/// Number of lines in a chunk
///
/// Used to specify how many lines a chunk spans.
/// Must be greater than 0.
#[nutype(
    validate(greater = 0),
    derive(Debug, Clone, Copy, Display, Serialize, Deserialize, PartialEq, Eq)
)]
pub struct LineCount(usize);

/// Maximum number of search results to return
///
/// Bounded between 1 and 100 to prevent excessive result sets.
#[nutype(
    validate(greater = 0, less_or_equal = 100),
    derive(Debug, Clone, Copy, Display, Serialize, Deserialize, PartialEq, Eq)
)]
pub struct ResultLimit(usize);

/// Relevance score for a search result
///
/// Normalized between 0.0 (not relevant) and 1.0 (highly relevant).
/// Used for ranking search results.
#[nutype(
    validate(greater_or_equal = 0.0, less_or_equal = 1.0),
    derive(
        Debug,
        Clone,
        Copy,
        Display,
        Serialize,
        Deserialize,
        PartialEq,
        PartialOrd
    )
)]
pub struct RelevanceScore(f32);

/// A term that matched in a search query
///
/// Used for highlighting matched terms in search results.
#[nutype(
    validate(not_empty),
    derive(Debug, Clone, Display, Serialize, Deserialize, PartialEq, Eq)
)]
pub struct MatchedTerm(String);

/// Function or type signature from Rust source
///
/// Provides additional context for Rust doc comments by including the
/// signature of the item being documented.
#[nutype(
    validate(not_empty),
    derive(Debug, Clone, Display, Serialize, Deserialize, PartialEq, Eq)
)]
pub struct Signature(String);

/// Semantic embedding vector for a chunk
///
/// A non-empty vector of floating-point values representing the semantic
/// meaning of a text chunk. Used for similarity search in Best mode.
#[nutype(
    validate(predicate = |v: &Vec<f32>| !v.is_empty()),
    derive(Debug, Clone, Serialize, Deserialize, PartialEq, AsRef, Deref)
)]
pub struct Embedding(Vec<f32>);

/// Raw content to be chunked
///
/// Represents the text content that will be split into searchable chunks.
/// No validation is applied as any text content is valid for chunking.
#[nutype(derive(Debug, Clone, Display, AsRef, Serialize, Deserialize, PartialEq, Eq))]
pub struct ChunkableContent(String);

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
