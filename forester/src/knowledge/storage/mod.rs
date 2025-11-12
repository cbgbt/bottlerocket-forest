//! Storage layer for chunk persistence

pub mod repository;
pub mod schema;

pub use repository::{ChunkRepository, IndexMetadata, StorageError};
pub use schema::SchemaError;
