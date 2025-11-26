//! Defines the chunking strategy trait and common types for file content processing.

use snafu::Snafu;
use std::path::Path;

use crate::knowledge::domain::{Chunk, ChunkSource, ChunkableContent, FileHash};

/// Strategy for chunking file content into searchable units
#[cfg_attr(test, mockall::automock)]
pub trait ChunkingStrategy: Send + Sync {
    /// Check if this strategy supports the given file
    fn supports(&self, file_path: &Path) -> bool;

    /// Chunk the file content into searchable units
    fn chunk(&self, input: &ChunkingInput) -> Result<Vec<Chunk>, ChunkingError>;
}

/// Input for chunking operations
#[derive(Debug, Clone, PartialEq)]
pub struct ChunkingInput {
    pub content: ChunkableContent,
    pub source: ChunkSource,
    /// Hash of the source file for content-addressed storage.
    pub file_hash: FileHash,
}

/// Errors that can occur during chunking
#[derive(Debug, Snafu, miette::Diagnostic)]
#[snafu(module, visibility(pub))]
pub enum ChunkingError {
    #[snafu(display("Failed to initialize tokenizer"))]
    #[diagnostic(
        code(sembly::chunking::tokenizer_init_error),
        help("Check that the embedding model configuration is valid")
    )]
    TokenizerInitError {
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },

    #[snafu(display("Failed to parse content from {file_path}"))]
    #[diagnostic(
        code(sembly::chunking::parse_error),
        help("The file may contain unsupported syntax (e.g., negative trait impls)")
    )]
    ParseError {
        file_path: String,
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },

    #[snafu(display("Token limit exceeded: {actual} > {max}"))]
    #[diagnostic(
        code(sembly::chunking::token_limit_exceeded),
        help("The content section is too large and cannot be chunked further")
    )]
    TokenLimitExceeded { actual: usize, max: usize },

    #[snafu(display("Invalid UTF-8 in content"))]
    #[diagnostic(
        code(sembly::chunking::invalid_utf8),
        help("The file contains invalid UTF-8 encoding")
    )]
    InvalidUtf8 { source: std::str::Utf8Error },
}
