//! Indexing orchestration for building and maintaining the knowledge index
//!
//! This module provides the high-level orchestration for creating and updating
//! the knowledge index from forest documentation files.
//!
//! # Components
//!
//! * [`FileScanner`] - Discovers indexable files (.md, .rs) in the forest
//! * [`Indexer`] - Unified indexer with multiple strategies (Build, Rebuild, Incremental)
//! * [`provider`] - Embedding generation for semantic search
//! * [`config`] - Configuration loading from `.forester.toml`
//! * [`filter`] - Filtering logic for controlling what gets indexed
//!
//! # Workflow
//!
//! The typical indexing workflow:
//! 1. Scan the forest to find documentation files
//! 2. Chunk files using appropriate strategies (markdown/rustdoc)
//! 3. Generate embeddings via [`IndexDataProvider`]
//! 4. Store indexed chunks in the repository
//!
//! For incremental updates, the indexer compares current files against
//! indexed files to identify additions, modifications, and deletions.

pub mod config;
pub mod filter;
pub mod indexer;
pub mod provider;
pub mod scanner;

pub use config::{ForesterConfig, ForesterConfigError, load_forester_config};
pub use filter::{IndexingFilter, RustFilter, RustItemType};
pub use indexer::{IndexResult, IndexStrategy, Indexer, IndexingError};
pub use provider::{IndexDataError, IndexDataProvider};
pub use scanner::{FileScanner, IndexableFile, ScanError};

// Re-export for indexer module
pub(crate) use crate::knowledge::chunking::DispatchError;
