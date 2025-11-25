//! Splits documentation files into searchable chunks for semantic indexing.
//!
//! Provides strategies for chunking markdown and Rust source files while preserving
//! structural context (heading hierarchies, item metadata). Uses token-aware splitting
//! with configurable overlap to respect embedding model constraints.

pub mod dispatcher;
pub mod markdown;
pub mod rustdoc;
pub mod strategy;

pub use dispatcher::{ChunkingDispatcher, DispatchError};
pub use markdown::MarkdownChunker;
pub use rustdoc::RustDocChunker;
pub use strategy::{ChunkingError, ChunkingInput, ChunkingStrategy};
