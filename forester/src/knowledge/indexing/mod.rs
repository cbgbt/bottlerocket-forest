//! Indexing orchestration for building and maintaining the knowledge index
//!
//! This module provides the high-level orchestration for creating and updating
//! the knowledge index from forest documentation files.
//!
//! # Components
//!
//! * [`FileScanner`] - Discovers indexable files (.md, .rs) in the forest
//! * [`Indexer`] - Unified indexer with multiple strategies (Build, Rebuild, Incremental)
//! * [`IndexBuilder`] - Legacy builder (use `Indexer` with `IndexStrategy::Build`)
//! * [`IncrementalUpdater`] - Legacy updater (use `Indexer` with `IndexStrategy::Incremental`)
//!
//! # Workflow
//!
//! The typical indexing workflow:
//! 1. Scan the forest to find documentation files
//! 2. Chunk files using appropriate strategies (markdown/rustdoc)
//! 3. Generate index data (BM25 terms or embeddings)
//! 4. Store indexed chunks in the repository
//!
//! For incremental updates, the indexer compares current files against
//! indexed files to identify additions, modifications, and deletions.

pub mod builder;
pub mod indexer;
pub mod scanner;
pub mod updater;

pub use builder::{IndexBuildError, IndexBuildResult, IndexBuilder};
pub use indexer::{IndexError, IndexResult, IndexStrategy, Indexer};
pub use scanner::{FileScanner, IndexableFile, ScanError};
pub use updater::{IncrementalUpdater, UpdateError, UpdateResult};

// Re-export for indexer module
pub(crate) use crate::knowledge::chunking::DispatchError;
