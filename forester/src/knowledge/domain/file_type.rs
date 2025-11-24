//! File type classification for indexing
//!
//! The file scanner uses [`FileType::from_path`] to classify files by extension,
//! then filters to indexable types before dispatching to chunking strategies.

use std::path::Path;

/// File classification determining indexing strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileType {
    Markdown,
    Rust,
    #[serde(skip)]
    Unsupported,
}

impl FileType {
    pub fn from_path(path: &Path) -> Self {
        match path.extension().and_then(|s| s.to_str()) {
            Some("md") => Self::Markdown,
            Some("rs") => Self::Rust,
            _ => Self::Unsupported,
        }
    }

    pub fn is_indexable(&self) -> bool {
        matches!(self, Self::Markdown | Self::Rust)
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
