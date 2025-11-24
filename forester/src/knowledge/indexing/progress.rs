//! Progress reporting for indexing operations
//!
//! Provides trait-based abstraction for reporting progress during index building.
//! The [`ProgressReporter`] trait defines lifecycle events for scanning, chunking,
//! and embedding phases.
//!
//! # Design
//!
//! Progress reporting is optional and injected via `Option<Box<dyn ProgressReporter>>`.
//! This allows:
//! - Zero-cost when disabled (None)
//! - Testability via mock implementations
//! - Multiple UI implementations (CLI, web, JSON, etc.)
//!
//! # Example
//!
//! ```no_run
//! use forester::knowledge::indexing::ProgressReporter;
//! use std::path::Path;
//!
//! struct MyReporter;
//!
//! impl ProgressReporter for MyReporter {
//!     fn scanning_started(&self) {
//!         println!("Starting scan...");
//!     }
//!     
//!     fn file_discovered(&self, path: &Path) {
//!         println!("Found: {}", path.display());
//!     }
//!     
//!     // ... implement other methods
//! #   fn scanning_completed(&self) {}
//! #   fn chunking_started(&self, _total_files: usize) {}
//! #   fn file_chunked(&self, _path: &Path, _chunk_count: usize) {}
//! #   fn chunking_completed(&self, _total_chunks: usize) {}
//! #   fn embedding_started(&self, _total_chunks: usize) {}
//! #   fn embeddings_generated(&self, _chunk_count: usize) {}
//! #   fn embedding_completed(&self) {}
//! #   fn indexing_completed(&self) {}
//! }
//! ```

use std::path::Path;

/// Reports progress during indexing operations
///
/// Implementations receive callbacks at key points during index building:
/// scanning, chunking, embedding, and completion. All methods are infallible -
/// implementations must handle their own errors internally.
///
/// The trait is `Send + Sync` to support parallel processing with rayon.
pub trait ProgressReporter: Send + Sync {
    /// Called when file scanning begins
    ///
    /// Signals the start of the file discovery phase. The total number of files
    /// is unknown at this point.
    fn scanning_started(&self);

    /// Called when a file is discovered during scanning
    ///
    /// Invoked for each file found that matches the scan criteria. May be called
    /// many times in rapid succession.
    fn file_discovered(&self, path: &Path);

    /// Called when file scanning completes
    ///
    /// Signals the end of file discovery. The total file count is now known.
    fn scanning_completed(&self);

    /// Called when chunking begins for all files
    ///
    /// Signals the start of the chunking phase. The total number of files to
    /// process is provided.
    fn chunking_started(&self, total_files: usize);

    /// Called when a file has been chunked
    ///
    /// Invoked after a file is successfully split into chunks. The chunk count
    /// for this specific file is provided.
    fn file_chunked(&self, path: &Path, chunk_count: usize);

    /// Called when all chunking is complete
    ///
    /// Signals the end of the chunking phase. The total number of chunks created
    /// across all files is provided.
    fn chunking_completed(&self, total_chunks: usize);

    /// Called when embedding generation begins
    ///
    /// Signals the start of the embedding phase. The total number of chunks to
    /// embed is provided.
    fn embedding_started(&self, total_chunks: usize);

    /// Called when embeddings for a batch of chunks are generated
    ///
    /// Invoked after each batch of embeddings is created. The number of chunks
    /// in this batch is provided. May be called multiple times as embeddings
    /// are generated in batches.
    fn embeddings_generated(&self, chunk_count: usize);

    /// Called when all embeddings are complete
    ///
    /// Signals the end of the embedding phase. All chunks now have embeddings.
    fn embedding_completed(&self);

    /// Called when the entire indexing operation completes
    ///
    /// Signals successful completion of the entire indexing operation. This is
    /// the final callback in the lifecycle.
    fn indexing_completed(&self);
}

/// No-op progress reporter
///
/// Implements [`ProgressReporter`] with empty methods. Useful for:
/// - Testing without progress output
/// - Library usage where progress is not needed
/// - Explicit opt-out of progress reporting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SilentReporter;

impl ProgressReporter for SilentReporter {
    fn scanning_started(&self) {}
    fn file_discovered(&self, _path: &Path) {}
    fn scanning_completed(&self) {}
    fn chunking_started(&self, _total_files: usize) {}
    fn file_chunked(&self, _path: &Path, _chunk_count: usize) {}
    fn chunking_completed(&self, _total_chunks: usize) {}
    fn embedding_started(&self, _total_chunks: usize) {}
    fn embeddings_generated(&self, _chunk_count: usize) {}
    fn embedding_completed(&self) {}
    fn indexing_completed(&self) {}
}
