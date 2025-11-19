//! Error types for embedding operations
//!
//! Defines errors that can occur during embedding model loading and
//! embedding generation for semantic search.

use snafu::Snafu;

/// Errors that can occur during embedding operations
#[derive(Debug, Snafu, miette::Diagnostic)]
#[snafu(module, visibility(pub(crate)))]
pub enum EmbeddingError {
    #[snafu(display("Failed to load embedding model: {model_name}"))]
    #[diagnostic(
        code(forester::embeddings::model_load_failed),
        help("Check that the model name is correct and the model files are accessible")
    )]
    ModelLoadFailed {
        model_name: String,
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },

    #[snafu(display("Failed to download model from: {url}"))]
    #[diagnostic(
        code(forester::embeddings::model_download_failed),
        help("Check your internet connection and that the URL is accessible")
    )]
    ModelDownloadFailed {
        url: String,
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },

    #[snafu(display("Failed to generate embedding for text"))]
    #[diagnostic(
        code(forester::embeddings::embedding_generation_failed),
        help("The text may be too long or contain unsupported characters")
    )]
    EmbeddingGenerationFailed {
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },

    #[snafu(display("Invalid embedding dimension: expected {expected}, got {actual}"))]
    #[diagnostic(
        code(forester::embeddings::dimension_mismatch),
        help("The model configuration may be incorrect or the model may have changed")
    )]
    DimensionMismatch { expected: usize, actual: usize },

    #[snafu(display("Model cache directory not accessible: {path}"))]
    #[diagnostic(
        code(forester::embeddings::cache_access_failed),
        help("Check directory permissions and available disk space")
    )]
    CacheAccessFailed {
        path: String,
        source: std::io::Error,
    },
}
