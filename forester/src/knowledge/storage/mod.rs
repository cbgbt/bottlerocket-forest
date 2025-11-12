//! Storage layer for chunk persistence

pub mod repository;
pub mod schema;
pub mod sqlite;

pub use repository::{ChunkRepository, IndexMetadata, StorageError};
pub use schema::SchemaError;
pub use sqlite::SqliteChunkRepository;
