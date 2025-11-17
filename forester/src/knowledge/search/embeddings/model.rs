//! Embedding model and provider trait
//!
//! Defines the interface for generating embeddings from text and provides
//! a builder for configuring and loading embedding models.

use std::path::PathBuf;

use fastembed::{EmbeddingModel as FastEmbedModel, InitOptions, TextEmbedding};
use snafu::IntoError;

use crate::knowledge::domain::Embedding;

use super::error::{EmbeddingError, embedding_error};

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
pub struct EmbeddingModel {
    #[builder(default = crate::knowledge::constants::DEFAULT_TOKENIZER_MODEL.to_string())]
    model_name: String,
    #[builder(default = crate::knowledge::constants::EMBEDDING_DIM)]
    dimension: usize,
    #[builder(default = default_cache_dir())]
    cache_dir: PathBuf,
}

impl EmbeddingModel {
    /// Load the embedding model with the configured parameters
    ///
    /// Downloads and caches the model if not already present.
    pub fn load(self) -> Result<LoadedEmbeddingModel, EmbeddingError> {
        let fastembed_model = map_model_name(&self.model_name)?;

        let init_options = InitOptions::new(fastembed_model)
            .with_cache_dir(self.cache_dir.clone())
            .with_show_download_progress(false);

        let text_embedding = TextEmbedding::try_new(init_options).map_err(|e| {
            embedding_error::ModelLoadFailedSnafu {
                model_name: self.model_name.clone(),
            }
            .into_error(Box::new(std::io::Error::other(e.to_string())))
        })?;

        Ok(LoadedEmbeddingModel {
            model_name: self.model_name,
            dimension: self.dimension,
            text_embedding,
        })
    }
}

/// A loaded embedding model ready for generating embeddings
pub struct LoadedEmbeddingModel {
    model_name: String,
    dimension: usize,
    text_embedding: TextEmbedding,
}

impl std::fmt::Debug for LoadedEmbeddingModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedEmbeddingModel")
            .field("model_name", &self.model_name)
            .field("dimension", &self.dimension)
            .field("text_embedding", &"<TextEmbedding>")
            .finish()
    }
}

impl EmbeddingProvider for LoadedEmbeddingModel {
    fn embed(&self, text: &str) -> Result<Embedding, EmbeddingError> {
        let embeddings = self
            .text_embedding
            .embed(vec![text.to_string()], None)
            .map_err(|e| {
                embedding_error::EmbeddingGenerationFailedSnafu
                    .into_error(Box::new(std::io::Error::other(e.to_string())))
            })?;

        let embedding_vec = embeddings.into_iter().next().ok_or_else(|| {
            embedding_error::EmbeddingGenerationFailedSnafu
                .into_error(Box::new(std::io::Error::other("no embedding returned")))
        })?;

        validate_dimension(&embedding_vec, self.dimension)?;

        Embedding::try_new(embedding_vec).map_err(|_| {
            embedding_error::EmbeddingGenerationFailedSnafu
                .into_error(Box::new(std::io::Error::other("invalid embedding vector")))
        })
    }

    fn embed_batch(&self, texts: Vec<String>) -> Result<Vec<Embedding>, EmbeddingError> {
        let embeddings = self.text_embedding.embed(texts, None).map_err(|e| {
            embedding_error::EmbeddingGenerationFailedSnafu
                .into_error(Box::new(std::io::Error::other(e.to_string())))
        })?;

        embeddings
            .into_iter()
            .map(|vec| {
                validate_dimension(&vec, self.dimension)?;
                Embedding::try_new(vec).map_err(|_| {
                    embedding_error::EmbeddingGenerationFailedSnafu
                        .into_error(Box::new(std::io::Error::other("invalid embedding vector")))
                })
            })
            .collect()
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn model_name(&self) -> &str {
        &self.model_name
    }
}

fn default_cache_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".forester")
        .join("cache")
        .join("model")
}

fn map_model_name(model_name: &str) -> Result<FastEmbedModel, EmbeddingError> {
    match model_name {
        "sentence-transformers/all-MiniLM-L6-v2" => Ok(FastEmbedModel::AllMiniLML6V2),
        "sentence-transformers/all-MiniLM-L12-v2" => Ok(FastEmbedModel::AllMiniLML12V2),
        _ => Err(embedding_error::ModelLoadFailedSnafu {
            model_name: model_name.to_string(),
        }
        .into_error(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("unsupported model: {}", model_name),
        )))),
    }
}

fn validate_dimension(embedding: &[f32], expected: usize) -> Result<(), EmbeddingError> {
    snafu::ensure!(
        embedding.len() == expected,
        embedding_error::DimensionMismatchSnafu {
            expected,
            actual: embedding.len()
        }
    );
    Ok(())
}
