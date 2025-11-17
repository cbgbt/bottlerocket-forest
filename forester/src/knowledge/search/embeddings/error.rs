//! Error types for embedding operations
//!
//! Defines errors that can occur during embedding model loading and
//! embedding generation for semantic search.

use snafu::Snafu;

/// Errors that can occur during embedding operations
#[derive(Debug, Snafu)]
#[snafu(module, visibility(pub(crate)))]
pub enum EmbeddingError {
    #[snafu(display("Failed to load embedding model: {model_name}"))]
    ModelLoadFailed {
        model_name: String,
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },

    #[snafu(display("Failed to download model from: {url}"))]
    ModelDownloadFailed {
        url: String,
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },

    #[snafu(display("Failed to generate embedding for text"))]
    EmbeddingGenerationFailed {
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },

    #[snafu(display("Invalid embedding dimension: expected {expected}, got {actual}"))]
    DimensionMismatch { expected: usize, actual: usize },

    #[snafu(display("Model cache directory not accessible: {path}"))]
    CacheAccessFailed {
        path: String,
        source: std::io::Error,
    },
}
