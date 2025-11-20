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

    /// Number of files skipped due to parse errors
    pub files_skipped: usize,

    /// Total chunks affected (created, updated, or removed)
    pub chunks_affected: usize,

    /// Time taken
    pub duration: Duration,

    /// Index mode used
    pub mode: IndexMode,
}

/// Errors that can occur during indexing
#[derive(Debug, Snafu, miette::Diagnostic)]
#[snafu(module, visibility(pub(crate)))]
pub enum IndexingError {
    #[snafu(display("Failed to scan files in forest directory"))]
    #[diagnostic(
        code(forester::indexing::scan_failed),
        help("Check that the directory is readable and contains valid Bottlerocket repositories")
    )]
    ScanFailed { source: ScanError },

    #[snafu(display("Failed to chunk file into searchable segments"))]
    #[diagnostic(
        code(forester::indexing::chunking_failed),
        help("The file may contain invalid syntax or unsupported content")
    )]
    ChunkingFailed { source: DispatchError },

    #[snafu(display("Failed to generate search index data for chunks"))]
    #[diagnostic(
        code(forester::indexing::index_data_generation_failed),
        help("This may be due to embedding model issues or invalid text content")
    )]
    IndexDataGenerationFailed { source: IndexDataError },

    #[snafu(display("Failed to save indexed chunks to database"))]
    #[diagnostic(
        code(forester::indexing::storage_failed),
        help("Check available disk space and database permissions")
    )]
    StorageFailed { source: StorageError },
}
