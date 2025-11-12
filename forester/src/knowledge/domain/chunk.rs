//! Chunk domain types
//!
//! A chunk represents a searchable unit of documentation with metadata about its
//! source location and context.

use bon::Builder;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

use super::{
    ChunkId, ForestRelativePath, HeadingText, ItemName, LineCount, LineNumber, RepoName, Signature,
    TokenCount,
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
    pub embedding: Option<Vec<f32>>,
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
    pub line_count: LineCount,
}

impl LineRange {
    /// Calculate the ending line number (inclusive)
    pub fn end(&self) -> LineNumber {
        let end_val = self.start.into_inner() + self.line_count.into_inner() - 1;
        LineNumber::try_new(end_val)
            .expect("end calculation should always produce valid line number")
    }
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
    fn test_line_range_single_line() {
        // Given A single line range
        let start = LineNumber::try_new(10).unwrap();
        let count = LineCount::try_new(1).unwrap();

        // When Building the line range
        let range = LineRange::builder().start(start).line_count(count).build();

        // Then End should equal start
        assert_eq!(range.end(), start);
    }

    #[test]
    fn test_line_range_multiple_lines() {
        // Given A multi-line range
        let start = LineNumber::try_new(10).unwrap();
        let count = LineCount::try_new(5).unwrap();

        // When Building the line range
        let range = LineRange::builder().start(start).line_count(count).build();

        // Then End should be start + count - 1
        assert_eq!(range.end(), LineNumber::try_new(14).unwrap());
    }
}
