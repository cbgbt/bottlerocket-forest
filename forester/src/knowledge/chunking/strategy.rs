//! Chunking strategy trait and types

use bon::Builder;
use snafu::Snafu;
use std::path::Path;

use crate::knowledge::domain::{Chunk, ChunkSource, ChunkableContent, TokenCount};

/// Strategy for chunking file content into searchable units
#[cfg_attr(test, mockall::automock)]
pub trait ChunkingStrategy {
    /// Check if this strategy supports the given file
    fn supports(&self, file_path: &Path) -> bool;

    /// Chunk the file content into searchable units
    fn chunk(&self, input: &ChunkingInput) -> Result<Vec<Chunk>, ChunkingError>;
}

/// Input for chunking operations
#[derive(Debug, Clone, PartialEq, Builder)]
#[builder(on(_, into))]
#[non_exhaustive]
pub struct ChunkingInput {
    pub content: ChunkableContent,
    pub source: ChunkSource,
    pub max_tokens: TokenCount,
}

/// Errors that can occur during chunking
#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum ChunkingError {
    #[snafu(display("Failed to parse content"))]
    ParseError {
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },

    #[snafu(display("Token limit exceeded: {actual} > {max}"))]
    TokenLimitExceeded { actual: usize, max: usize },

    #[snafu(display("Invalid UTF-8 in content"))]
    InvalidUtf8 { source: std::str::Utf8Error },
}
