//! Storage layer for chunk persistence

pub mod repository;

pub use repository::{ChunkRepository, IndexMetadata, StorageError};
