//! Search implementations for the knowledge index

pub mod bm25;
pub mod embeddings;
pub mod engine;

pub use bm25::Bm25SearchEngine;
pub use embeddings::{
    EmbeddingError, EmbeddingModel, EmbeddingProvider, LoadedEmbeddingModel, VectorOps,
};
pub use engine::{SearchEngine, SearchError};
