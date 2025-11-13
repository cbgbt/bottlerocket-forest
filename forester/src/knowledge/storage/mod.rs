//! Storage layer for chunk persistence
//!
//! This module provides:
//! - `repository`: Trait definition for chunk storage operations
//! - `schema`: Database schema definitions
//! - `sqlite`: SQLite implementation with submodules for serialization, queries, and search

pub mod repository;
pub mod schema;
pub mod sqlite;

pub use repository::{ChunkRepository, IndexMetadata, StorageError};
pub use schema::SchemaError;
pub use sqlite::SqliteChunkRepository;
