//! Chunking strategies for splitting documentation into searchable units

pub mod markdown;
pub mod strategy;

pub use markdown::MarkdownChunker;
pub use strategy::{ChunkingError, ChunkingInput, ChunkingStrategy};
