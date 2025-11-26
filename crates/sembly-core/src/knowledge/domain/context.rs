//! Context domain types for multi-context indexing.
//!
//! A context represents a registered working directory within a workspace.
//! Contexts enable multiple directories to share a single embedding database
//! while maintaining isolated file mappings.

use bon::Builder;
use chrono::{DateTime, Utc};
use path_clean::PathClean;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};

/// Unique identifier for a context, representing a canonicalized relative path.
///
/// Context IDs are normalized to ensure consistent representation:
/// - Leading `./` is stripped
/// - `.` and `..` components are resolved
/// - The default context at workspace root is represented as `.`
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContextId(String);

impl ContextId {
    /// Creates a new ContextId from a path, applying canonicalization.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, ContextIdError> {
        let path = path.as_ref();
        let s = path.to_str().ok_or(ContextIdError::InvalidUtf8)?;
        let cleaned = PathBuf::from(s).clean();
        let result = cleaned.to_str().ok_or(ContextIdError::InvalidUtf8)?;

        if result.starts_with('/') {
            return Err(ContextIdError::AbsolutePath);
        }
        if result == ".." || result.starts_with("../") {
            return Err(ContextIdError::PathEscapesRoot);
        }

        let normalized = if result.is_empty() { "." } else { result };
        Ok(Self(normalized.to_string()))
    }

    /// Returns the context ID as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ContextId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Errors that can occur when creating a ContextId.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextIdError {
    /// Path contains non-UTF-8 characters.
    InvalidUtf8,
    /// Path attempts to escape the workspace root with `..` components.
    PathEscapesRoot,
    /// Path is absolute, but context IDs must be relative.
    AbsolutePath,
}

impl fmt::Display for ContextIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUtf8 => write!(f, "Path contains invalid UTF-8"),
            Self::PathEscapesRoot => write!(f, "Path escapes workspace root"),
            Self::AbsolutePath => write!(f, "Context ID must be a relative path"),
        }
    }
}

impl std::error::Error for ContextIdError {}

/// A registered context within a workspace.
///
/// Contexts track which files belong to a specific working directory.
/// Multiple contexts can share the same underlying embeddings through
/// content-addressed storage.
#[derive(Debug, Clone, PartialEq, Eq, Builder)]
#[builder(on(_, into))]
#[non_exhaustive]
pub struct Context {
    /// Unique identifier for this context.
    pub context_id: ContextId,
    /// When this context was first registered.
    #[builder(default = Utc::now())]
    pub created_at: DateTime<Utc>,
    /// When this context was last indexed, if ever.
    pub last_indexed: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod test {
    use super::*;
    use test_case::test_case;

    #[test_case("./foo", "foo" ; "strips leading dot-slash")]
    #[test_case("foo/../bar", "bar" ; "resolves parent directory")]
    #[test_case(".", "." ; "preserves single dot")]
    #[test_case("worktrees/feature-a", "worktrees/feature-a" ; "preserves nested paths")]
    fn canonicalizes_path(input: &str, expected: &str) {
        // Given a path input
        // When creating a ContextId
        let result = ContextId::from_path(input).unwrap();

        // Then it should canonicalize to the expected value
        assert_eq!(result.as_str(), expected);
    }

    #[test_case(".." ; "single parent")]
    #[test_case("foo/../../bar" ; "escapes via nested parent")]
    fn rejects_paths_escaping_root(input: &str) {
        // Given a path that escapes the workspace root
        // When creating a ContextId
        let result = ContextId::from_path(input);

        // Then it should fail with PathEscapesRoot error
        assert_eq!(result, Err(ContextIdError::PathEscapesRoot));
    }

    #[test]
    fn rejects_absolute_paths() {
        // Given an absolute path
        let input = "/absolute/path";

        // When creating a ContextId
        let result = ContextId::from_path(input);

        // Then it should fail with AbsolutePath error
        assert_eq!(result, Err(ContextIdError::AbsolutePath));
    }
}
