//! Filtering rules for controlling indexing scope
//!
//! Provides [`IndexingFilter`] for file-level filtering and [`RustFilter`] for
//! Rust-specific filtering based on visibility, item type, and documentation
//! length. Filters are typically constructed from configuration loaded via
//! [`CrumblyConfig`](super::config::CrumblyConfig).

use serde::{Deserialize, Serialize};

use crate::knowledge::domain::{DocLineCount, Visibility};

/// Categories of Rust language items that can be filtered during indexing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RustItemType {
    /// Module declarations.
    #[serde(rename = "modules")]
    Module,
    /// Function definitions.
    #[serde(rename = "functions")]
    Function,
    /// Struct definitions.
    #[serde(rename = "structs")]
    Struct,
    /// Enum definitions.
    #[serde(rename = "enums")]
    Enum,
    /// Trait definitions.
    #[serde(rename = "traits")]
    Trait,
    /// Impl blocks.
    #[serde(rename = "impls")]
    Impl,
    /// Type alias definitions.
    #[serde(rename = "type-aliases")]
    TypeAlias,
    /// Constant definitions.
    #[serde(rename = "constants")]
    Constant,
}

/// Categories of Java language items that can be filtered during indexing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JavaItemType {
    /// All item types.
    All,
    /// Class definitions.
    #[serde(rename = "classes")]
    Class,
    /// Interface definitions.
    #[serde(rename = "interfaces")]
    Interface,
    /// Enum definitions.
    #[serde(rename = "enums")]
    Enum,
    /// Record definitions.
    #[serde(rename = "records")]
    Record,
    /// Method definitions.
    #[serde(rename = "methods")]
    Method,
    /// Field definitions.
    #[serde(rename = "fields")]
    Field,
    /// Constructor definitions.
    #[serde(rename = "constructors")]
    Constructor,
    /// Annotation definitions.
    #[serde(rename = "annotations")]
    Annotation,
}

/// Categories of Go language items that can be filtered during indexing
///
/// Unlike RustItemType, includes an `All` variant for convenience in configuration.
/// This allows `items = ["all"]` in config rather than listing all types explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GoItemType {
    /// All item types.
    All,
    /// Function definitions.
    #[serde(rename = "functions")]
    Function,
    /// Method definitions.
    #[serde(rename = "methods")]
    Method,
    /// Struct definitions.
    #[serde(rename = "structs")]
    Struct,
    /// Interface definitions.
    #[serde(rename = "interfaces")]
    Interface,
    /// Type definitions.
    #[serde(rename = "types")]
    Type,
    /// Constant definitions.
    #[serde(rename = "constants")]
    Const,
    /// Variable definitions.
    #[serde(rename = "variables")]
    Var,
}

/// Filtering rules for Rust source code indexing
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustFilter {
    visibility: Vec<Visibility>,
    items: Vec<RustItemType>,
    min_doc_lines: DocLineCount,
}

impl RustFilter {
    /// Create a filter with visibility, item types, and minimum documentation length
    pub fn new(
        visibility: Vec<Visibility>,
        items: Vec<RustItemType>,
        min_doc_lines: DocLineCount,
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
        doc_lines: DocLineCount,
    ) -> bool {
        doc_lines >= self.min_doc_lines
            && self.visibility.contains(visibility)
            && self.items.contains(item_type)
    }
}

/// Filtering rules for Java source code indexing
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaFilter {
    visibility: Vec<Visibility>,
    items: Vec<JavaItemType>,
    min_doc_lines: DocLineCount,
}

impl JavaFilter {
    /// Create a filter with visibility, item types, and minimum documentation length
    pub fn new(
        visibility: Vec<Visibility>,
        items: Vec<JavaItemType>,
        min_doc_lines: DocLineCount,
    ) -> Self {
        Self {
            visibility,
            items,
            min_doc_lines,
        }
    }

    /// Determine whether a Java item should be indexed based on filter criteria
    pub fn should_index(
        &self,
        visibility: &Visibility,
        item_type: &JavaItemType,
        doc_lines: DocLineCount,
    ) -> bool {
        doc_lines >= self.min_doc_lines
            && self.visibility.contains(visibility)
            && (self.items.contains(&JavaItemType::All) || self.items.contains(item_type))
    }
}

/// Filtering rules for Go source code indexing
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoFilter {
    visibility: Vec<Visibility>,
    items: Vec<GoItemType>,
    min_doc_lines: DocLineCount,
}

impl GoFilter {
    /// Create a filter with visibility, item types, and minimum documentation length
    pub fn new(
        visibility: Vec<Visibility>,
        items: Vec<GoItemType>,
        min_doc_lines: DocLineCount,
    ) -> Self {
        Self {
            visibility,
            items,
            min_doc_lines,
        }
    }

    /// Determine whether a Go item should be indexed based on filter criteria
    pub fn should_index(
        &self,
        visibility: &Visibility,
        item_type: &GoItemType,
        doc_lines: DocLineCount,
    ) -> bool {
        doc_lines >= self.min_doc_lines
            && self.visibility.contains(visibility)
            && (self.items.contains(&GoItemType::All) || self.items.contains(item_type))
    }
}

/// Combined filtering rules for file types and language-specific criteria
#[derive(Debug, Clone)]
pub struct IndexingFilter {
    enabled_file_types: Vec<crate::knowledge::domain::FileType>,
    rust_filter: Option<RustFilter>,
    go_filter: Option<GoFilter>,
    java_filter: Option<JavaFilter>,
}

impl IndexingFilter {
    /// Create a filter with enabled file types and optional language-specific rules
    pub fn new(
        enabled_file_types: Vec<crate::knowledge::domain::FileType>,
        rust_filter: Option<RustFilter>,
        go_filter: Option<GoFilter>,
        java_filter: Option<JavaFilter>,
    ) -> Self {
        Self {
            enabled_file_types,
            rust_filter,
            go_filter,
            java_filter,
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

    /// Access the Go-specific filter if configured
    pub fn go_filter(&self) -> Option<&GoFilter> {
        self.go_filter.as_ref()
    }

    /// Access the Java-specific filter if configured
    pub fn java_filter(&self) -> Option<&JavaFilter> {
        self.java_filter.as_ref()
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
                DocLineCount::new(0),
            )),
            go_filter: Some(GoFilter::new(
                vec![Visibility::Public, Visibility::Private],
                vec![
                    GoItemType::Function,
                    GoItemType::Method,
                    GoItemType::Struct,
                    GoItemType::Interface,
                    GoItemType::Type,
                    GoItemType::Const,
                    GoItemType::Var,
                ],
                DocLineCount::new(0),
            )),
            java_filter: Some(JavaFilter::new(
                vec![Visibility::Public, Visibility::Private],
                vec![
                    JavaItemType::Class,
                    JavaItemType::Interface,
                    JavaItemType::Enum,
                    JavaItemType::Record,
                    JavaItemType::Method,
                    JavaItemType::Field,
                    JavaItemType::Constructor,
                    JavaItemType::Annotation,
                ],
                DocLineCount::new(0),
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
        let filter = RustFilter::new(
            vec![Visibility::Public],
            vec![RustItemType::Function],
            DocLineCount::new(3),
        );

        // When Checking items with different doc line counts
        let short_doc = filter.should_index(
            &Visibility::Public,
            &RustItemType::Function,
            DocLineCount::new(2),
        );
        let long_doc = filter.should_index(
            &Visibility::Public,
            &RustItemType::Function,
            DocLineCount::new(5),
        );

        // Then Only long docs should pass
        assert!(!short_doc);
        assert!(long_doc);
    }

    #[test]
    fn test_rust_filter_should_index_checks_visibility() {
        // Given A filter for public items only
        let filter = RustFilter::new(
            vec![Visibility::Public],
            vec![RustItemType::Function],
            DocLineCount::new(0),
        );

        // When Checking items with different visibility
        let public = filter.should_index(
            &Visibility::Public,
            &RustItemType::Function,
            DocLineCount::new(100),
        );
        let private = filter.should_index(
            &Visibility::Private,
            &RustItemType::Function,
            DocLineCount::new(100),
        );

        // Then Only public items should pass
        assert!(public);
        assert!(!private);
    }

    #[test]
    fn test_rust_filter_should_index_checks_item_type() {
        // Given A filter for structs only
        let filter = RustFilter::new(
            vec![Visibility::Public],
            vec![RustItemType::Struct],
            DocLineCount::new(0),
        );

        // When Checking different item types
        let struct_item = filter.should_index(
            &Visibility::Public,
            &RustItemType::Struct,
            DocLineCount::new(100),
        );
        let function_item = filter.should_index(
            &Visibility::Public,
            &RustItemType::Function,
            DocLineCount::new(100),
        );

        // Then Only structs should pass
        assert!(struct_item);
        assert!(!function_item);
    }

    #[test]
    fn test_indexing_filter_should_index_file_type() {
        // Given A filter with only markdown enabled
        use crate::knowledge::domain::FileType;
        let filter = IndexingFilter::new(vec![FileType::Markdown], None, None, None);

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
        let rust_filter = RustFilter::new(vec![Visibility::Public], vec![], DocLineCount::new(0));
        let filter = IndexingFilter::new(vec![], Some(rust_filter), None, None);

        // When Getting the Rust filter
        let result = filter.rust_filter();

        // Then It should return a reference
        assert!(result.is_some());
    }
}

#[test]
fn test_go_filter_should_index_checks_doc_lines() {
    // Given a filter requiring minimum 3 doc lines
    let filter = GoFilter::new(
        vec![Visibility::Public],
        vec![GoItemType::Function],
        DocLineCount::new(3),
    );

    // When checking items with different doc line counts
    let short_doc = filter.should_index(
        &Visibility::Public,
        &GoItemType::Function,
        DocLineCount::new(2),
    );
    let long_doc = filter.should_index(
        &Visibility::Public,
        &GoItemType::Function,
        DocLineCount::new(5),
    );

    // Then only items meeting the threshold should be indexed
    assert!(!short_doc);
    assert!(long_doc);
}

#[test]
fn test_go_filter_should_index_checks_visibility() {
    // Given a filter accepting only public items
    let filter = GoFilter::new(
        vec![Visibility::Public],
        vec![GoItemType::Function],
        DocLineCount::new(0),
    );

    // When checking items with different visibilities
    let public = filter.should_index(
        &Visibility::Public,
        &GoItemType::Function,
        DocLineCount::new(100),
    );
    let private = filter.should_index(
        &Visibility::Private,
        &GoItemType::Function,
        DocLineCount::new(100),
    );

    // Then only public items should be indexed
    assert!(public);
    assert!(!private);
}

#[test]
fn test_go_filter_should_index_checks_item_type() {
    // Given a filter accepting only struct items
    let filter = GoFilter::new(
        vec![Visibility::Public],
        vec![GoItemType::Struct],
        DocLineCount::new(0),
    );

    // When checking items with different types
    let struct_item = filter.should_index(
        &Visibility::Public,
        &GoItemType::Struct,
        DocLineCount::new(100),
    );
    let function_item = filter.should_index(
        &Visibility::Public,
        &GoItemType::Function,
        DocLineCount::new(100),
    );

    // Then only struct items should be indexed
    assert!(struct_item);
    assert!(!function_item);
}
