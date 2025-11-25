//! Search implementations for the knowledge index
//!
//! This module provides semantic search capabilities using embedding-based similarity.
//!
//! ## Component Overview
//!
//! * [`engine`]: Core search engine trait and error types
//! * [`semantic`]: Semantic search implementation using embeddings
//! * [`embeddings`]: Embedding generation and vector operations

pub mod embeddings;
pub mod engine;
pub mod semantic;

pub use embeddings::{
    EmbeddingError, EmbeddingModel, EmbeddingProvider, LoadedEmbeddingModel, VectorOps,
};
pub use engine::{SearchEngine, SearchError};
pub use semantic::SemanticSearchEngine;
