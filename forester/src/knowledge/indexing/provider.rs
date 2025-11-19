//! Index data generation providers
//!
//! This module defines the unified interface for generating index data from text chunks.
//! Different providers implement different indexing strategies (BM25 keyword indexing,
//! semantic embeddings, etc.) while presenting a consistent API.
//!
//! ## Architecture
//!
//! The [`IndexDataProvider`] trait abstracts over different indexing strategies:
//! - [`Bm25Provider`]: Generates BM25 term frequencies for keyword search
//! - [`EmbeddingDataProvider`]: Generates semantic embeddings for similarity search
//!
//! ## Cross-References
//!
//! - BM25 tokenization: [`super::bm25::calculate_bm25_terms`]
//! - Semantic embeddings: [`crate::knowledge::search::embeddings::EmbeddingProvider`]

use snafu::{IntoError, Snafu};

use crate::knowledge::domain::{IndexData, IndexMode};
use crate::knowledge::search::embeddings::EmbeddingProvider;

use super::bm25::calculate_bm25_terms;

/// Generates index data from text content
///
/// Implementations of this trait convert raw text into searchable index data.
/// The specific format depends on the indexing strategy (BM25 terms, embeddings, etc.).
#[cfg_attr(test, mockall::automock)]
pub trait IndexDataProvider: Send + Sync {
    /// Generate index data from text
    ///
    /// Processes the input text and returns index-specific data wrapped in [`IndexData`].
    fn generate(&self, text: &str) -> Result<IndexData, IndexDataError>;

    /// Returns the index mode for this provider
    fn mode(&self) -> IndexMode;
}

/// Errors that can occur during index data generation
#[derive(Debug, Snafu, miette::Diagnostic)]
#[snafu(module)]
pub enum IndexDataError {
    #[snafu(display("Failed to calculate BM25 terms"))]
    #[diagnostic(
        code(forester::indexing::bm25_failed),
        help("The text may contain unsupported characters or be malformed")
    )]
    Bm25Failed {
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },

    #[snafu(display("Failed to generate embedding"))]
    #[diagnostic(
        code(forester::indexing::embedding_failed),
        help("The embedding model may not be loaded or the text may be invalid")
    )]
    EmbeddingFailed {
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
}

/// BM25 keyword indexing provider
///
/// Generates term frequency maps for keyword-based search using the BM25 algorithm.
/// This provider tokenizes text, removes stopwords, and counts term frequencies.
///
/// For the search implementation that uses these terms, see the BM25 search
/// functionality in the storage layer.
pub struct Bm25Provider;

impl IndexDataProvider for Bm25Provider {
    fn generate(&self, text: &str) -> Result<IndexData, IndexDataError> {
        let bm25_terms = calculate_bm25_terms(text);
        Ok(IndexData::Fast { bm25_terms })
    }

    fn mode(&self) -> IndexMode {
        IndexMode::Fast
    }
}

/// Semantic embedding provider
///
/// Generates semantic embeddings for similarity-based search using neural language models.
/// This provider wraps an [`EmbeddingProvider`] to convert text into dense vector
/// representations that capture semantic meaning.
///
/// For the search implementation that uses these embeddings, see the semantic search
/// functionality in the storage layer.
pub struct EmbeddingDataProvider {
    embedding_provider: Box<dyn EmbeddingProvider>,
}

impl EmbeddingDataProvider {
    /// Create a new embedding data provider
    ///
    /// Wraps an embedding provider that will be used to generate embeddings
    /// for text chunks during indexing.
    pub fn new(embedding_provider: Box<dyn EmbeddingProvider>) -> Self {
        Self { embedding_provider }
    }
}

impl IndexDataProvider for EmbeddingDataProvider {
    fn generate(&self, text: &str) -> Result<IndexData, IndexDataError> {
        let embedding = self
            .embedding_provider
            .embed(text)
            .map_err(|e| index_data_error::EmbeddingFailedSnafu.into_error(Box::new(e)))?;

        Ok(IndexData::Best { embedding })
    }

    fn mode(&self) -> IndexMode {
        IndexMode::Best
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::knowledge::domain::Embedding;
    use crate::knowledge::search::embeddings::model::MockEmbeddingProvider;

    #[test]
    fn test_bm25_provider_generates_fast_index_data() {
        // Given A Bm25Provider and sample text
        let provider = Bm25Provider;
        let text = "rust programming language";

        // When Generating index data
        let result = provider.generate(text);

        // Then It should return Fast mode IndexData
        assert!(result.is_ok());
        let index_data = result.unwrap();
        assert!(matches!(index_data, IndexData::Fast { .. }));
    }

    #[test]
    fn test_bm25_provider_calculates_correct_terms() {
        // Given A Bm25Provider and text with known terms
        let provider = Bm25Provider;
        let text = "rust rust programming";

        // When Generating index data
        let result = provider.generate(text).unwrap();

        // Then The BM25 terms should be correctly calculated
        if let IndexData::Fast { bm25_terms } = result {
            assert_eq!(bm25_terms.get("rust"), Some(&2));
            assert_eq!(bm25_terms.get("programming"), Some(&1));
            assert_eq!(bm25_terms.len(), 2);
        } else {
            panic!("Expected Fast mode IndexData");
        }
    }

    #[test]
    fn test_bm25_provider_mode_returns_fast() {
        // Given A Bm25Provider
        let provider = Bm25Provider;

        // When Getting the mode
        let mode = provider.mode();

        // Then It should return Fast mode
        assert_eq!(mode, IndexMode::Fast);
    }

    #[test]
    fn test_bm25_provider_handles_empty_text() {
        // Given A Bm25Provider and empty text
        let provider = Bm25Provider;
        let text = "";

        // When Generating index data
        let result = provider.generate(text).unwrap();

        // Then It should return empty BM25 terms
        if let IndexData::Fast { bm25_terms } = result {
            assert!(bm25_terms.is_empty());
        } else {
            panic!("Expected Fast mode IndexData");
        }
    }

    #[test]
    fn test_embedding_provider_generates_best_index_data() {
        // Given An EmbeddingDataProvider with mock embedding provider
        let mut mock = MockEmbeddingProvider::new();
        let test_embedding = Embedding::try_new(vec![0.1, 0.2, 0.3]).unwrap();
        mock.expect_embed()
            .returning(move |_| Ok(test_embedding.clone()));

        let provider = EmbeddingDataProvider::new(Box::new(mock));
        let text = "rust programming language";

        // When Generating index data
        let result = provider.generate(text);

        // Then It should return Best mode IndexData
        assert!(result.is_ok());
        let index_data = result.unwrap();
        assert!(matches!(index_data, IndexData::Best { .. }));
    }

    #[test]
    fn test_embedding_provider_wraps_embedding_correctly() {
        // Given An EmbeddingDataProvider with mock returning specific embedding
        let mut mock = MockEmbeddingProvider::new();
        let expected_embedding = Embedding::try_new(vec![0.5, 0.6, 0.7]).unwrap();
        let expected_clone = expected_embedding.clone();
        mock.expect_embed()
            .returning(move |_| Ok(expected_clone.clone()));

        let provider = EmbeddingDataProvider::new(Box::new(mock));
        let text = "test content";

        // When Generating index data
        let result = provider.generate(text).unwrap();

        // Then The embedding should be correctly wrapped in IndexData::Best
        if let IndexData::Best { embedding } = result {
            assert_eq!(embedding, expected_embedding);
        } else {
            panic!("Expected Best mode IndexData");
        }
    }

    #[test]
    fn test_embedding_provider_mode_returns_best() {
        // Given An EmbeddingDataProvider
        let mock = MockEmbeddingProvider::new();
        let provider = EmbeddingDataProvider::new(Box::new(mock));

        // When Getting the mode
        let mode = provider.mode();

        // Then It should return Best mode
        assert_eq!(mode, IndexMode::Best);
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

        // When Generating index data
        let result = provider.generate(text);

        // Then It should return an EmbeddingFailed error
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            IndexDataError::EmbeddingFailed { .. }
        ));
    }
}
