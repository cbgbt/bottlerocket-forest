//! Storage layer for chunk persistence
//!
//! This module provides:
//! - `repository`: Trait definition for chunk storage operations
//! - `schema`: Database schema definitions
//! - `sqlite`: SQLite implementation with submodules for serialization, queries, and search
//! - `bm25`: BM25 term calculation utilities

pub mod bm25;
pub mod repository;
pub mod schema;
pub mod sqlite;

pub use bm25::calculate_bm25_terms;
pub use repository::{ChunkRepository, IndexMetadata, StorageError};
pub use schema::SchemaError;
pub use sqlite::SqliteChunkRepository;
