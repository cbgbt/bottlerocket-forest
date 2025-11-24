//! Filtering rules for controlling indexing scope
//!
//! Provides [`IndexingFilter`] for file-level filtering and [`RustFilter`] for
//! Rust-specific filtering based on visibility, item type, and documentation
//! length. Filters are typically constructed from configuration loaded via
//! [`ForesterConfig`](super::config::ForesterConfig).

use serde::{Deserialize, Serialize};

use crate::knowledge::domain::Visibility;

/// Categories of Rust language items that can be filtered during indexing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RustItemType {
    #[serde(rename = "modules")]
    Module,
    #[serde(rename = "functions")]
    Function,
    #[serde(rename = "structs")]
    Struct,
    #[serde(rename = "enums")]
    Enum,
    #[serde(rename = "traits")]
    Trait,
    #[serde(rename = "impls")]
    Impl,
    #[serde(rename = "type-aliases")]
    TypeAlias,
    #[serde(rename = "constants")]
    Constant,
}

/// Filtering rules for Rust source code indexing
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustFilter {
    visibility: Vec<Visibility>,
    items: Vec<RustItemType>,
    min_doc_lines: usize,
}

impl RustFilter {
    /// Create a filter with visibility, item types, and minimum documentation length
    pub fn new(
        visibility: Vec<Visibility>,
        items: Vec<RustItemType>,
        min_doc_lines: usize,
    ) -> Self {
        Self {
            visibility,
            items,
            min_doc_lines,
        }
    }

    /// Determine whether a Rust item should be indexed based on filter criteria
    pub fn should_index(
        &self,
        visibility: &Visibility,
        item_type: &RustItemType,
        doc_lines: usize,
    ) -> bool {
        if doc_lines < self.min_doc_lines {
            return false;
        }

        if !self.visibility.contains(visibility) {
            return false;
        }

        if !self.items.contains(item_type) {
            return false;
        }

        true
    }
}

/// Combined filtering rules for file types and Rust-specific criteria
#[derive(Debug, Clone)]
pub struct IndexingFilter {
    enabled_file_types: Vec<crate::knowledge::domain::FileType>,
    rust_filter: Option<RustFilter>,
}

impl IndexingFilter {
    /// Create a filter with enabled file types and optional Rust-specific rules
    pub fn new(
        enabled_file_types: Vec<crate::knowledge::domain::FileType>,
        rust_filter: Option<RustFilter>,
    ) -> Self {
        Self {
            enabled_file_types,
            rust_filter,
        }
    }

    /// Determine whether a file type should be indexed
    pub fn should_index_file_type(&self, file_type: crate::knowledge::domain::FileType) -> bool {
        self.enabled_file_types.contains(&file_type)
    }

    /// Access the Rust-specific filter if configured
    pub fn rust_filter(&self) -> Option<&RustFilter> {
        self.rust_filter.as_ref()
    }
}

impl Default for IndexingFilter {
    fn default() -> Self {
        use crate::knowledge::domain::FileType;

        Self {
            enabled_file_types: vec![FileType::Markdown, FileType::Rust],
            rust_filter: Some(RustFilter::new(
                vec![Visibility::Public],
                vec![
                    RustItemType::Module,
                    RustItemType::Function,
                    RustItemType::Struct,
                    RustItemType::Enum,
                    RustItemType::Trait,
                    RustItemType::Impl,
                    RustItemType::TypeAlias,
                    RustItemType::Constant,
                ],
                0,
            )),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_rust_filter_should_index_checks_doc_lines() {
        // Given A filter with minimum doc lines
        let filter = RustFilter::new(vec![Visibility::Public], vec![RustItemType::Function], 3);

        // When Checking items with different doc line counts
        let short_doc = filter.should_index(&Visibility::Public, &RustItemType::Function, 2);
        let long_doc = filter.should_index(&Visibility::Public, &RustItemType::Function, 5);

        // Then Only long docs should pass
        assert!(!short_doc);
        assert!(long_doc);
    }

    #[test]
    fn test_rust_filter_should_index_checks_visibility() {
        // Given A filter for public items only
        let filter = RustFilter::new(vec![Visibility::Public], vec![RustItemType::Function], 0);

        // When Checking items with different visibility
        let public = filter.should_index(&Visibility::Public, &RustItemType::Function, 100);
        let private = filter.should_index(&Visibility::Private, &RustItemType::Function, 100);

        // Then Only public items should pass
        assert!(public);
        assert!(!private);
    }

    #[test]
    fn test_rust_filter_should_index_checks_item_type() {
        // Given A filter for structs only
        let filter = RustFilter::new(vec![Visibility::Public], vec![RustItemType::Struct], 0);

        // When Checking different item types
        let struct_item = filter.should_index(&Visibility::Public, &RustItemType::Struct, 100);
        let function_item = filter.should_index(&Visibility::Public, &RustItemType::Function, 100);

        // Then Only structs should pass
        assert!(struct_item);
        assert!(!function_item);
    }

    #[test]
    fn test_indexing_filter_should_index_file_type() {
        // Given A filter with only markdown enabled
        use crate::knowledge::domain::FileType;
        let filter = IndexingFilter::new(vec![FileType::Markdown], None);

        // When Checking different file types
        let markdown = filter.should_index_file_type(FileType::Markdown);
        let rust = filter.should_index_file_type(FileType::Rust);

        // Then Only markdown should pass
        assert!(markdown);
        assert!(!rust);
    }

    #[test]
    fn test_indexing_filter_rust_filter_returns_reference() {
        // Given A filter with Rust filter configured
        let rust_filter = RustFilter::new(vec![Visibility::Public], vec![], 0);
        let filter = IndexingFilter::new(vec![], Some(rust_filter));

        // When Getting the Rust filter
        let result = filter.rust_filter();

        // Then It should return a reference
        assert!(result.is_some());
    }
}
