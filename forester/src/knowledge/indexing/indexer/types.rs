//! Types for indexing operations

use bon::Builder;
use snafu::Snafu;
use std::time::Duration;

use crate::knowledge::domain::IndexMode;
use crate::knowledge::storage::StorageError;

use super::super::{DispatchError, IndexDataError, ScanError};

/// Result of an indexing operation
#[derive(Debug, Clone, PartialEq, Builder)]
#[non_exhaustive]
pub struct IndexResult {
    /// Number of files processed (files_added + files_updated)
    pub files_processed: usize,

    /// Number of files added (Build/Rebuild: all files, Incremental: new files only)
    pub files_added: usize,

    /// Number of files updated (Build/Rebuild: 0, Incremental: modified files)
    pub files_updated: usize,

    /// Number of files removed (Build/Rebuild: 0, Incremental: deleted files)
    pub files_removed: usize,

    /// Total chunks affected (created, updated, or removed)
    pub chunks_affected: usize,

    /// Time taken
    pub duration: Duration,

    /// Index mode used
    pub mode: IndexMode,
}

/// Errors that can occur during indexing
#[derive(Debug, Snafu)]
#[snafu(module, visibility(pub(crate)))]
pub enum IndexingError {
    #[snafu(display("Failed to scan files"))]
    ScanFailed { source: ScanError },

    #[snafu(display("Failed to chunk file"))]
    ChunkingFailed { source: DispatchError },

    #[snafu(display("Failed to generate index data"))]
    IndexDataGenerationFailed { source: IndexDataError },

    #[snafu(display("Failed to access storage"))]
    StorageFailed { source: StorageError },
}
