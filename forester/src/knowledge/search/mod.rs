//! Search implementations for the knowledge index

pub mod bm25;
pub mod engine;

pub use bm25::Bm25SearchEngine;
pub use engine::{SearchEngine, SearchError};
