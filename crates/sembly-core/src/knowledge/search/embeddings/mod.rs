//! Embedding generation and vector operations for semantic search
//!
//! This module provides abstractions for generating embeddings from text and
//! performing vector operations for semantic similarity search.
//!
//! ## Module Organization
//!
//! - [`model`]: Embedding provider trait and model implementations
//! - [`operations`]: Vector operations (cosine similarity, normalization, etc.)
//! - [`error`]: Error types for embedding operations

pub mod error;
pub mod model;
pub mod operations;

pub use error::EmbeddingError;
pub use model::{EmbeddingModel, EmbeddingProvider, LoadedEmbeddingModel};
pub use operations::VectorOps;
