//! File type classification for indexing
//!
//! Determines which files should be indexed and how they should be processed.
//! The file scanner uses [`FileType::from_path`] to classify files, then filters
//! to only indexable types before dispatching to appropriate chunking strategies.

use std::path::Path;

/// Classification of files for indexing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    /// Markdown documentation files
    Markdown,
    /// Rust source files (for doc comment extraction)
    Rust,
    /// Files that should not be indexed
    Unsupported,
}

impl FileType {
    /// Classify a file by its path
    pub fn from_path(path: &Path) -> Self {
        match path.extension().and_then(|s| s.to_str()) {
            Some("md") => Self::Markdown,
            Some("rs") => Self::Rust,
            _ => Self::Unsupported,
        }
    }

    /// Check if this file type should be indexed
    pub fn is_indexable(&self) -> bool {
        matches!(self, Self::Markdown)
        // TODO: Re-enable Rust once we handle parse errors gracefully
        // matches!(self, Self::Markdown | Self::Rust)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use test_case::test_case;

    #[test_case("README.md", FileType::Markdown, true ; "markdown file")]
    #[test_case("src/main.rs", FileType::Rust, true ; "rust file")]
    #[test_case("Cargo.toml", FileType::Unsupported, false ; "toml file")]
    #[test_case("LICENSE", FileType::Unsupported, false ; "no extension")]
    fn test_file_classification(path: &str, expected_type: FileType, expected_indexable: bool) {
        // Given A file path
        let path = Path::new(path);

        // When Classifying the file
        let file_type = FileType::from_path(path);

        // Then It should match expected classification
        assert_eq!(file_type, expected_type);
        assert_eq!(file_type.is_indexable(), expected_indexable);
    }
}
