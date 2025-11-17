//! Embedding model and provider trait
//!
//! Defines the interface for generating embeddings from text and provides
//! a builder for configuring and loading embedding models.

use crate::knowledge::domain::Embedding;

use super::error::EmbeddingError;

/// Generates embeddings for text content
#[cfg_attr(test, mockall::automock)]
pub trait EmbeddingProvider: Send + Sync {
    /// Generate an embedding for a single text
    fn embed(&self, text: &str) -> Result<Embedding, EmbeddingError>;

    /// Generate embeddings for multiple texts in a batch
    ///
    /// Batch processing can be more efficient than individual calls.
    fn embed_batch(&self, texts: Vec<String>) -> Result<Vec<Embedding>, EmbeddingError>;

    /// Get the dimensionality of embeddings produced by this provider
    fn dimension(&self) -> usize;

    /// Get the model name used by this provider
    fn model_name(&self) -> &str;
}

/// Embedding model configuration using sentence-transformers
///
/// Use the builder to configure model parameters, then call `load()` to
/// download and initialize the model for generating embeddings.
#[derive(bon::Builder)]
#[builder(on(_, into))]
#[non_exhaustive]
#[allow(dead_code)]
pub struct EmbeddingModel {
    #[builder(default = crate::knowledge::constants::DEFAULT_TOKENIZER_MODEL.to_string())]
    model_name: String,
    #[builder(default = crate::knowledge::constants::EMBEDDING_DIM)]
    dimension: usize,
}

impl EmbeddingModel {
    /// Load the embedding model with the configured parameters
    ///
    /// Downloads and caches the model if not already present.
    pub fn load(self) -> Result<LoadedEmbeddingModel, EmbeddingError> {
        todo!()
    }
}

/// A loaded embedding model ready for generating embeddings
pub struct LoadedEmbeddingModel {
    model_name: String,
    dimension: usize,
}

impl EmbeddingProvider for LoadedEmbeddingModel {
    fn embed(&self, _text: &str) -> Result<Embedding, EmbeddingError> {
        todo!()
    }

    fn embed_batch(&self, _texts: Vec<String>) -> Result<Vec<Embedding>, EmbeddingError> {
        todo!()
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn model_name(&self) -> &str {
        &self.model_name
    }
}
