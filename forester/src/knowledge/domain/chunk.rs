//! Chunk domain types
//!
//! A chunk represents a searchable unit of documentation with metadata about its
//! source location and context.

use bon::Builder;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::SystemTime;

use super::{
    ChunkId, ForestRelativePath, HeadingText, IndexMode, ItemName, LineCount, LineNumber, RepoName,
    Signature, TokenCount,
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
    pub indexed_at: SystemTime,
    pub index_data: IndexData,
}

/// Index-specific data for a chunk
///
/// This enum ensures type safety by making it impossible to create chunks
/// with mismatched index modes and data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IndexData {
    /// Fast mode using BM25 keyword search
    Fast { bm25_terms: BTreeMap<String, u32> },
    /// Best mode using semantic embeddings
    Best { embedding: Vec<f32> },
}

impl IndexData {
    /// Returns the index mode for this data
    pub fn mode(&self) -> IndexMode {
        match self {
            IndexData::Fast { .. } => IndexMode::Fast,
            IndexData::Best { .. } => IndexMode::Best,
        }
    }
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

    #[test]
    fn test_index_data_fast_mode() {
        // Given Fast mode index data
        let mut terms = BTreeMap::new();
        terms.insert("test".to_string(), 3);
        terms.insert("example".to_string(), 1);
        let index_data = IndexData::Fast {
            bm25_terms: terms.clone(),
        };

        // When Getting the mode
        let mode = index_data.mode();

        // Then It should be Fast
        assert_eq!(mode, IndexMode::Fast);

        // And The terms should be accessible
        if let IndexData::Fast { bm25_terms } = index_data {
            assert_eq!(bm25_terms.get("test"), Some(&3));
            assert_eq!(bm25_terms.get("example"), Some(&1));
        } else {
            panic!("Expected Fast variant");
        }
    }

    #[test]
    fn test_index_data_best_mode() {
        // Given Best mode index data
        let embedding = vec![0.1, 0.2, 0.3];
        let index_data = IndexData::Best {
            embedding: embedding.clone(),
        };

        // When Getting the mode
        let mode = index_data.mode();

        // Then It should be Best
        assert_eq!(mode, IndexMode::Best);

        // And The embedding should be accessible
        if let IndexData::Best { embedding: emb } = index_data {
            assert_eq!(emb, embedding);
        } else {
            panic!("Expected Best variant");
        }
    }

    #[test]
    fn test_chunk_with_fast_index_data() {
        // Given A chunk with Fast mode data
        let mut terms = BTreeMap::new();
        terms.insert("rust".to_string(), 2);

        let chunk = Chunk::builder()
            .id(ChunkId::new(uuid::Uuid::new_v4()))
            .source(
                ChunkSource::builder()
                    .file_path(ForestRelativePath::try_new("test.md").unwrap())
                    .repo_name(RepoName::try_new("test-repo").unwrap())
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
                    .text("test content")
                    .token_count(TokenCount::try_new(2).unwrap())
                    .build(),
            )
            .context(ChunkContext::Markdown(
                MarkdownContext::builder()
                    .heading_hierarchy(vec![HeadingText::try_new("Test").unwrap()])
                    .build(),
            ))
            .indexed_at(SystemTime::now())
            .index_data(IndexData::Fast { bm25_terms: terms })
            .build();

        // Then The chunk should have Fast mode
        assert_eq!(chunk.index_data.mode(), IndexMode::Fast);
    }

    #[test]
    fn test_chunk_with_best_index_data() {
        // Given A chunk with Best mode data
        let chunk = Chunk::builder()
            .id(ChunkId::new(uuid::Uuid::new_v4()))
            .source(
                ChunkSource::builder()
                    .file_path(ForestRelativePath::try_new("test.rs").unwrap())
                    .repo_name(RepoName::try_new("test-repo").unwrap())
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
                    .text("test content")
                    .token_count(TokenCount::try_new(2).unwrap())
                    .build(),
            )
            .context(ChunkContext::RustDoc(
                RustDocContext::builder()
                    .item_type(RustItemType::Function)
                    .item_name(ItemName::try_new("test_fn").unwrap())
                    .visibility(Visibility::Public)
                    .build(),
            ))
            .indexed_at(SystemTime::now())
            .index_data(IndexData::Best {
                embedding: vec![0.1, 0.2, 0.3],
            })
            .build();

        // Then The chunk should have Best mode
        assert_eq!(chunk.index_data.mode(), IndexMode::Best);
    }
}
