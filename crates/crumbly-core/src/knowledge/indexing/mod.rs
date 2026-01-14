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
//! * [`config`] - Configuration loading from `.crumbly.toml`
//! * [`filter`] - Filtering logic for controlling what gets indexed
//! * [`progress`] - Progress reporting abstraction
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
//!
//! # Progress Reporting
//!
//! Progress can be reported via the [`ProgressReporter`] trait. Use
//! [`SilentReporter`] for no-op progress or implement custom reporters
//! for different UIs.

pub mod cacher;
pub mod config;
pub mod filter;
pub mod indexer;
pub mod progress;
pub mod provider;
pub mod scanner;
pub mod source;

pub use cacher::{CacheError, CacheResult, ChunkCacher};
pub use config::{CrumblyConfig, CrumblyConfigError, load_crumbly_config};
pub use filter::{
    GoFilter, GoItemType, IndexingFilter, JavaFilter, JavaItemType, RustFilter, RustItemType,
};
pub use indexer::{BatchConfig, IndexResult, IndexStrategy, Indexer, IndexingError};
pub use progress::{ProgressReporter, SilentReporter};
pub use provider::{IndexDataError, IndexDataProvider};
pub use scanner::{FileScanner, IndexableFile, ScanError};
pub use source::{
    BareGitSource, BareGitSourceError, ContentEntry, ContentSource, GitBlobRef, GitRev,
};

// Re-export for indexer module
pub(crate) use crate::knowledge::chunking::DispatchError;
