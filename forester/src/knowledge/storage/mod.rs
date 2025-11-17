//! Storage layer for chunk persistence
//!
//! This module implements dual-mode indexing for documentation chunks using SQLite.
//!
//! # Architecture
//!
//! The storage layer consists of:
//!
//! * **Repository Trait** (`repository`): Abstract interface for chunk storage operations.
//!   Defines methods for saving, retrieving, searching, and managing chunks.
//!
//! * **SQLite Implementation** (`sqlite`): Concrete implementation using SQLite with:
//!   - Regular table for chunk metadata (content, source, context)
//!   - Virtual table (sqlite-vec) for vector embeddings (Best mode)
//!   - BM25 term storage in JSON (Fast mode)
//!

pub mod repository;
pub mod schema;
pub mod sqlite;

pub use repository::{ChunkRepository, StorageError};
pub use schema::SchemaError;
pub use sqlite::SqliteChunkRepository;
