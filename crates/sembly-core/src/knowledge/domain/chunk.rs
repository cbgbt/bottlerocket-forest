//! Chunk domain types
//!
//! Core abstractions:
//! * [`Chunk`] combines source, content, and context
//! * [`ChunkSource`] identifies origin file and repository
//! * [`ChunkContent`] contains text and token count
//! * [`ChunkContext`] provides type-specific metadata

use bon::Builder;
use serde::{Deserialize, Serialize};

use super::{ChunkId, ForestRelativePath, HeadingText, ItemName, RepoName, Signature, TokenCount};
use crate::knowledge::indexing::RustItemType;

/// Searchable documentation unit with source and context metadata
#[derive(Debug, Clone, PartialEq, Builder, Serialize, Deserialize)]
#[builder(on(_, into))]
#[non_exhaustive]
pub struct Chunk {
    pub id: ChunkId,
    pub source: ChunkSource,
    pub content: ChunkContent,
    pub context: ChunkContext,
}

/// Origin location of a chunk
#[derive(Debug, Clone, PartialEq, Eq, Builder, Serialize, Deserialize)]
#[builder(on(_, into))]
#[non_exhaustive]
pub struct ChunkSource {
    pub file_path: ForestRelativePath,
    pub repo_name: RepoName,
}

/// Text content with token count for size tracking
#[derive(Debug, Clone, PartialEq, Eq, Builder, Serialize, Deserialize)]
#[builder(on(_, into))]
#[non_exhaustive]
pub struct ChunkContent {
    pub text: String,
    pub token_count: TokenCount,
}

/// File-type-specific metadata for chunks
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ChunkContext {
    Markdown(MarkdownContext),
    RustDoc(RustDocContext),
}

/// Heading hierarchy for markdown document structure
#[derive(Debug, Clone, PartialEq, Eq, Builder, Serialize, Deserialize)]
#[builder(on(_, into))]
#[non_exhaustive]
pub struct MarkdownContext {
    pub heading_hierarchy: Vec<HeadingText>,
}

/// Rust item metadata for doc comment context
#[derive(Debug, Clone, PartialEq, Eq, Builder, Serialize, Deserialize)]
#[builder(on(_, into))]
#[non_exhaustive]
pub struct RustDocContext {
    pub item_name: ItemName,
    pub visibility: Visibility,
    pub signature: Option<Signature>,
    pub item_type: RustItemType,
}

/// Visibility of a Rust item
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
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
