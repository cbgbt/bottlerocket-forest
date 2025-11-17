//! Chunking strategies for splitting documentation into searchable units

pub mod dispatcher;
pub mod markdown;
pub mod rustdoc;
pub mod strategy;

pub use dispatcher::{ChunkingDispatcher, DispatchError};
pub use markdown::MarkdownChunker;
pub use rustdoc::RustDocChunker;
pub use strategy::{ChunkingError, ChunkingInput, ChunkingStrategy};
