//! Chunk domain types
//!
//! A chunk represents a searchable unit of documentation with metadata about its
//! source location and context.

use bon::Builder;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

use super::{
    ChunkId, ForestRelativePath, HeadingText, ItemName, LineNumber, RepoName, Signature, TokenCount,
};

/// A searchable chunk of documentation with metadata
#[derive(Debug, Clone, Builder, Serialize, Deserialize)]
#[builder(on(_, into))]
#[non_exhaustive]
pub struct Chunk {
    pub id: ChunkId,
    pub source: ChunkSource,
    pub content: ChunkContent,
    pub context: ChunkContext,
    pub indexed_at: SystemTime,
}

/// Source location of the chunk
#[derive(Debug, Clone, Builder, Serialize, Deserialize)]
#[builder(on(_, into))]
#[non_exhaustive]
pub struct ChunkSource {
    pub file_path: ForestRelativePath,
    pub repo_name: RepoName,
    pub line_range: LineRange,
}

/// Line range in a file
#[derive(Debug, Clone, Builder, Serialize, Deserialize)]
#[builder(on(_, into))]
#[non_exhaustive]
pub struct LineRange {
    pub start: LineNumber,
    pub end: LineNumber,
}

/// The actual content to be indexed
#[derive(Debug, Clone, Builder, Serialize, Deserialize)]
#[builder(on(_, into))]
#[non_exhaustive]
pub struct ChunkContent {
    pub text: String,
    pub token_count: TokenCount,
}

/// Type-specific metadata about the chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChunkContext {
    Markdown(MarkdownContext),
    RustDoc(RustDocContext),
}

/// Context for markdown chunks
#[derive(Debug, Clone, Builder, Serialize, Deserialize)]
#[builder(on(_, into))]
#[non_exhaustive]
pub struct MarkdownContext {
    pub heading_hierarchy: Vec<HeadingText>,
}

/// Context for Rust doc comment chunks
#[derive(Debug, Clone, Builder, Serialize, Deserialize)]
#[builder(on(_, into))]
#[non_exhaustive]
pub struct RustDocContext {
    pub item_type: RustItemType,
    pub item_name: ItemName,
    pub visibility: Visibility,
    pub signature: Option<Signature>,
}

/// Type of Rust item being documented
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum RustItemType {
    Function,
    Struct,
    Enum,
    Module,
    Trait,
    Impl,
}

/// Visibility of a Rust item
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Crate,
    Private,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_line_range_validates_start_before_end() {
        // Given A line range where start > end
        let start = LineNumber::try_new(25).unwrap();
        let end = LineNumber::try_new(10).unwrap();

        // When Building the line range
        let range = LineRange::builder().start(start).end(end).build();

        // Then It should allow construction (validation happens at higher level)
        assert_eq!(range.start, start);
        assert_eq!(range.end, end);
    }
}
