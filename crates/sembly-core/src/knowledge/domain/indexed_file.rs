//! IndexedFile domain type for tracking files within contexts.
//!
//! An IndexedFile represents a file's presence in a specific context,
//! linking the file path to its content hash for content-addressed storage.

use bon::Builder;

use super::{ContextId, FileHash, ForestRelativePath};

/// A file tracked within a specific context.
///
/// IndexedFile records associate files with contexts and their content hashes,
/// enabling content-addressed storage where identical content shares embeddings
/// across multiple contexts.
#[derive(Debug, Clone, PartialEq, Eq, Builder)]
#[non_exhaustive]
pub struct IndexedFile {
    /// The context this file belongs to.
    #[builder(into)]
    pub context_id: ContextId,
    /// Path to the file relative to the workspace root.
    #[builder(into)]
    pub file_path: ForestRelativePath,
    /// Content hash of the file for deduplication.
    #[builder(into)]
    pub file_hash: FileHash,
    /// File modification time in nanoseconds since Unix epoch.
    pub mtime_ns: i64,
}
