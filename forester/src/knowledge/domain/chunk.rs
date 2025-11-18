//! Chunk domain types
//!
//! A chunk represents a searchable unit of documentation with metadata about its
//! source location and context.
//!
//! Core abstractions:
//! * [`Chunk`] - The main type combining source, content, and context
//! * [`ChunkSource`] - Identifies where the chunk came from (file, repo, line range)
//! * [`ChunkContent`] - The actual text and token count
//! * [`ChunkContext`] - Type-specific metadata (markdown headings or Rust doc context)

use bon::Builder;
use serde::{Deserialize, Serialize};

use super::{
    ChunkId, ForestRelativePath, HeadingText, ItemName, LineCount, LineNumber, RepoName, Signature,
    TokenCount,
};

/// A searchable chunk of documentation with metadata
#[derive(Debug, Clone, PartialEq, Builder, Serialize, Deserialize)]
#[builder(on(_, into))]
#[non_exhaustive]
pub struct Chunk {
    pub id: ChunkId,
    pub source: ChunkSource,
    pub content: ChunkContent,
    pub context: ChunkContext,
}

/// Source location of the chunk
#[derive(Debug, Clone, PartialEq, Eq, Builder, Serialize, Deserialize)]
#[builder(on(_, into))]
#[non_exhaustive]
pub struct ChunkSource {
    pub file_path: ForestRelativePath,
    pub repo_name: RepoName,
    pub line_range: LineRange,
}

/// Line range in a file
#[derive(Debug, Clone, PartialEq, Eq, Builder, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Builder, Serialize, Deserialize)]
#[builder(on(_, into))]
#[non_exhaustive]
pub struct ChunkContent {
    pub text: String,
    pub token_count: TokenCount,
}

/// Type-specific metadata about the chunk
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ChunkContext {
    Markdown(MarkdownContext),
    RustDoc(RustDocContext),
}

/// Context for markdown chunks
#[derive(Debug, Clone, PartialEq, Eq, Builder, Serialize, Deserialize)]
#[builder(on(_, into))]
#[non_exhaustive]
pub struct MarkdownContext {
    pub heading_hierarchy: Vec<HeadingText>,
}

/// Context for Rust doc comment chunks
#[derive(Debug, Clone, PartialEq, Eq, Builder, Serialize, Deserialize)]
#[builder(on(_, into))]
#[non_exhaustive]
pub struct RustDocContext {
    pub item_name: ItemName,
    pub visibility: Visibility,
    pub signature: Option<Signature>,
}

/// Visibility of a Rust item
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Crate,
    Private,
}

impl From<syn::Visibility> for Visibility {
    fn from(vis: syn::Visibility) -> Self {
        match vis {
            syn::Visibility::Public(_) => Visibility::Public,
            syn::Visibility::Restricted(r) => {
                if r.path.is_ident("crate") {
                    Visibility::Crate
                } else {
                    Visibility::Private
                }
            }
            syn::Visibility::Inherited => Visibility::Private,
        }
    }
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
