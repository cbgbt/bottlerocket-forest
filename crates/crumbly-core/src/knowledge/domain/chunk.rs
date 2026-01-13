//! Chunk domain types
//!
//! Core abstractions:
//! * [`Chunk`] combines source, content, and context
//! * [`ChunkSource`] identifies origin file and repository
//! * [`ChunkContent`] contains text and token count
//! * [`ChunkContext`] provides type-specific metadata

use bon::Builder;
use serde::{Deserialize, Serialize};

use super::{
    ChunkHash, ChunkId, FileHash, HeadingText, IndexRelativePath, ItemName, PackageName, RepoName,
    Signature, TokenCount,
};
use crate::knowledge::indexing::RustItemType;

/// Searchable documentation unit with source and context metadata.
#[derive(Debug, Clone, PartialEq, Builder, Serialize, Deserialize)]
#[builder(on(_, into))]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
#[non_exhaustive]
pub struct Chunk {
    /// Unique identifier for this chunk.
    pub id: ChunkId,
    /// Content-addressed identifier for this chunk.
    pub chunk_hash: ChunkHash,
    /// Hash of the file that produced this chunk.
    pub file_hash: FileHash,
    /// Origin location of this chunk.
    pub source: ChunkSource,
    /// Text content with token count.
    pub content: ChunkContent,
    /// File-type-specific metadata.
    pub context: ChunkContext,
}

/// Origin location of a chunk.
#[derive(Debug, Clone, PartialEq, Eq, Builder, Serialize, Deserialize)]
#[builder(on(_, into))]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
#[non_exhaustive]
pub struct ChunkSource {
    /// Path to the source file relative to index root.
    pub file_path: IndexRelativePath,
    /// Name of the repository containing this chunk.
    pub repo_name: RepoName,
}

/// Text content with token count for size tracking.
#[derive(Debug, Clone, PartialEq, Eq, Builder, Serialize, Deserialize)]
#[builder(on(_, into))]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
#[non_exhaustive]
pub struct ChunkContent {
    /// Raw text content of the chunk.
    pub text: String,
    /// Number of tokens in the text.
    pub token_count: TokenCount,
}

/// File-type-specific metadata for chunks
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ChunkContext {
    /// Markdown document context.
    Markdown(MarkdownContext),
    /// Rust documentation context.
    RustDoc(RustDocContext),
    /// Go documentation context.
    GoDoc(GoDocContext),
    /// Unknown context type for forward compatibility.
    Unknown(UnknownContext),
}

/// Preserves unrecognized chunk context types for forward compatibility.
#[derive(Debug, Clone, PartialEq, Eq, Builder, Serialize, Deserialize)]
#[builder(on(_, into))]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
#[non_exhaustive]
pub struct UnknownContext {
    /// Original context type string from storage.
    pub type_name: String,
    /// Raw JSON data preserved for round-tripping.
    pub raw_data: String,
}

/// Heading hierarchy for markdown document structure.
#[derive(Debug, Clone, PartialEq, Eq, Builder, Serialize, Deserialize)]
#[builder(on(_, into))]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
#[non_exhaustive]
pub struct MarkdownContext {
    /// Nested heading path from document root to this chunk.
    pub heading_hierarchy: Vec<HeadingText>,
}

/// Rust item metadata for doc comment context.
#[derive(Debug, Clone, PartialEq, Eq, Builder, Serialize, Deserialize)]
#[builder(on(_, into))]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
#[non_exhaustive]
pub struct RustDocContext {
    /// Name of the documented item.
    pub item_name: ItemName,
    /// Visibility level of the item.
    pub visibility: Visibility,
    /// Function or type signature if applicable.
    pub signature: Option<Signature>,
    /// Kind of Rust item.
    pub item_type: RustItemType,
}

/// Visibility of a Rust item.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    /// Visible everywhere (`pub`).
    Public,
    /// Visible within the crate (`pub(crate)`).
    Crate,
    /// Visible only within the module.
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

/// Go item metadata for doc comment context.
#[derive(Debug, Clone, PartialEq, Eq, Builder, Serialize, Deserialize)]
#[builder(on(_, into))]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
#[non_exhaustive]
pub struct GoDocContext {
    /// Name of the documented item.
    pub item_name: ItemName,
    /// Visibility level of the item.
    pub visibility: GoVisibility,
    /// Function or type signature if applicable.
    pub signature: Option<Signature>,
    /// Kind of Go item.
    pub item_type: GoItemType,
    /// Package name for this Go item. None only during parsing errors or malformed files.
    pub package_name: Option<PackageName>,
}

/// Visibility of a Go item.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GoVisibility {
    /// Exported (capitalized) item visible outside package.
    Exported,
    /// Unexported (lowercase) item visible only within package.
    Unexported,
}

/// Type of Go item.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GoItemType {
    /// Standalone function.
    Function,
    /// Method on a type.
    Method,
    /// Struct type definition.
    Struct,
    /// Interface type definition.
    Interface,
    /// Type alias or definition.
    Type,
    /// Constant declaration.
    Const,
    /// Variable declaration.
    Var,
    /// Package-level documentation.
    Package,
}
