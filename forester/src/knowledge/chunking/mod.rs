//! Chunking strategies for splitting documentation into searchable units

pub mod strategy;

pub use strategy::{ChunkingError, ChunkingInput, ChunkingStrategy};
