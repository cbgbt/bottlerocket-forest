//! Embedding generation for semantic search indexing
//!
//! This module provides the [`IndexDataProvider`] trait for generating embeddings
//! from text content. The [`EmbeddingDataProvider`] implementation wraps an
//! [`EmbeddingProvider`] to convert text chunks into vector embeddings suitable
//! for semantic search.

use snafu::{IntoError, Snafu};

use super::ProgressReporter;
use crate::knowledge::domain::Embedding;
use crate::knowledge::search::embeddings::EmbeddingProvider;

/// Generates embeddings from text content for semantic search
#[cfg_attr(test, mockall::automock)]
#[allow(clippy::needless_lifetimes)]
pub trait IndexDataProvider: Send + Sync {
    /// Generate embedding vector from text
    fn generate(&self, text: &str) -> Result<Embedding, IndexDataError>;

    /// Generate embeddings for multiple texts in batch
    ///
    /// Implementations should use native batch processing when available for
    /// better performance. Batching typically provides significant speedup
    /// over individual generation calls.
    fn generate_batch<'a>(&self, texts: &[&'a str]) -> Result<Vec<Embedding>, IndexDataError>;

    /// Generate embedding with progress reporting
    ///
    /// Default implementation calls `generate()` and reports progress.
    /// Implementations can override for batch-aware progress reporting.
    fn generate_with_progress<'a>(
        &self,
        text: &str,
        progress: Option<&'a dyn ProgressReporter>,
    ) -> Result<Embedding, IndexDataError> {
        let result = self.generate(text)?;
        if let Some(progress) = progress {
            progress.embeddings_generated(1);
        }
        Ok(result)
    }

    /// Generate embeddings for multiple texts with progress reporting
    ///
    /// Default implementation calls `generate_batch()` and reports progress.
    /// Implementations can override for fine-grained batch progress reporting.
    fn generate_batch_with_progress<'a, 'b>(
        &self,
        texts: &[&'b str],
        progress: Option<&'a dyn ProgressReporter>,
    ) -> Result<Vec<Embedding>, IndexDataError> {
        let result = self.generate_batch(texts)?;
        if let Some(progress) = progress {
            progress.embeddings_generated(texts.len());
        }
        Ok(result)
    }
}

/// Errors that can occur during index data generation
#[derive(Debug, Snafu, miette::Diagnostic)]
#[snafu(module)]
pub enum IndexDataError {
    #[snafu(display("Failed to generate embedding"))]
    #[diagnostic(
        code(forester::indexing::embedding_failed),
        help("The embedding model may not be loaded or the text may be invalid")
    )]
    EmbeddingFailed {
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
}

/// Adapter for generating embeddings via an EmbeddingProvider
pub struct EmbeddingDataProvider {
    embedding_provider: Box<dyn EmbeddingProvider>,
}

impl EmbeddingDataProvider {
    /// Create a new embedding data provider wrapping an EmbeddingProvider
    pub fn new(embedding_provider: Box<dyn EmbeddingProvider>) -> Self {
        Self { embedding_provider }
    }
}

impl IndexDataProvider for EmbeddingDataProvider {
    fn generate(&self, text: &str) -> Result<Embedding, IndexDataError> {
        self.embedding_provider
            .embed(text)
            .map_err(|e| index_data_error::EmbeddingFailedSnafu.into_error(Box::new(e)))
    }

    #[allow(clippy::needless_lifetimes)]
    fn generate_batch<'a>(&self, texts: &[&'a str]) -> Result<Vec<Embedding>, IndexDataError> {
        let text_strings: Vec<String> = texts.iter().map(|s| s.to_string()).collect();

        self.embedding_provider
            .embed_batch(text_strings)
            .map_err(|e| index_data_error::EmbeddingFailedSnafu.into_error(Box::new(e)))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::knowledge::domain::Embedding;
    use crate::knowledge::search::embeddings::model::MockEmbeddingProvider;

    #[test]
    fn test_embedding_provider_generates_embedding() {
        // Given An EmbeddingDataProvider with mock embedding provider
        let mut mock = MockEmbeddingProvider::new();
        let test_embedding = Embedding::try_new(vec![0.1, 0.2, 0.3]).unwrap();
        mock.expect_embed()
            .returning(move |_| Ok(test_embedding.clone()));

        let provider = EmbeddingDataProvider::new(Box::new(mock));
        let text = "rust programming language";

        // When Generating embedding
        let result = provider.generate(text);

        // Then It should return an embedding
        assert!(result.is_ok());
    }

    #[test]
    fn test_embedding_provider_returns_correct_embedding() {
        // Given An EmbeddingDataProvider with mock returning specific embedding
        let mut mock = MockEmbeddingProvider::new();
        let expected_embedding = Embedding::try_new(vec![0.5, 0.6, 0.7]).unwrap();
        let expected_clone = expected_embedding.clone();
        mock.expect_embed()
            .returning(move |_| Ok(expected_clone.clone()));

        let provider = EmbeddingDataProvider::new(Box::new(mock));
        let text = "test content";

        // When Generating embedding
        let result = provider.generate(text).unwrap();

        // Then The embedding should match expected
        assert_eq!(result, expected_embedding);
    }

    #[test]
    fn test_embedding_provider_propagates_errors() {
        // Given An EmbeddingDataProvider with mock that returns error
        let mut mock = MockEmbeddingProvider::new();
        mock.expect_embed().returning(|_| {
            Err(
                crate::knowledge::search::embeddings::EmbeddingError::EmbeddingGenerationFailed {
                    source: Box::new(std::io::Error::other("test error")),
                },
            )
        });

        let provider = EmbeddingDataProvider::new(Box::new(mock));
        let text = "test content";

        // When Generating embedding
        let result = provider.generate(text);

        // Then It should return an EmbeddingFailed error
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            IndexDataError::EmbeddingFailed { .. }
        ));
    }
}
