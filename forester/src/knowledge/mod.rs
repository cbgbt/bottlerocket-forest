//! Knowledge indexing for Bottlerocket documentation
//!
//! This module provides semantic and keyword search across forest documentation,
//! enabling fast, targeted documentation lookup for AI agents and developers.
//!
//! # Architecture
//!
//! The knowledge index is organized into three layers:
//!
//! * **Domain Layer** (`domain/`): Type-safe domain models
//!   Defines core concepts like `Chunk`, `SearchQuery`, and `IndexMode` (Fast/Best).
//!
//! * **Storage Layer** (`storage/`): Knowledgebase persistence
//!
//! * **Chunking Layer** Splits documentation into searchable chunks with
//!   context preservation (markdown headings, rustdoc items).
//!
//! # Index Modes
//!
//! - **Fast**: BM25 lexical search - quick keyword matching, no embeddings
//! - **Best**: Semantic search using all-MiniLM-L6-v2 embeddings - understands meaning
//!

pub mod chunking;
pub mod constants;
pub mod domain;
pub mod storage;

pub use chunking::{ChunkingError, ChunkingInput, ChunkingStrategy};
pub use domain::{
    Chunk, ChunkContent, ChunkContext, ChunkId, ChunkSource, ChunkableContent, FileType, IndexMode,
    LineRange, MarkdownContext, RepoName, RustDocContext, SearchQuery, SearchResult, SearchResults,
};
pub use storage::{ChunkRepository, IndexMetadata, StorageError};
