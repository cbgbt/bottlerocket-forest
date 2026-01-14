//! Java documentation extraction utilities.

use tree_sitter::Node;

use crate::knowledge::domain::{JavaItemType, JavaVisibility, Visibility};
use crate::knowledge::indexing::JavaItemType as FilterJavaItemType;

/// Constants for tree-sitter Java node kinds.
pub mod node_kinds {
    pub const BLOCK_COMMENT: &str = "block_comment";
    pub const MODIFIERS: &str = "modifiers";
    pub const CLASS_DECLARATION: &str = "class_declaration";
    pub const INTERFACE_DECLARATION: &str = "interface_declaration";
    pub const ENUM_DECLARATION: &str = "enum_declaration";
    pub const RECORD_DECLARATION: &str = "record_declaration";
    pub const METHOD_DECLARATION: &str = "method_declaration";
    pub const CONSTRUCTOR_DECLARATION: &str = "constructor_declaration";
    pub const FIELD_DECLARATION: &str = "field_declaration";
    pub const ANNOTATION_TYPE_DECLARATION: &str = "annotation_type_declaration";
}

/// Extracts Javadoc comment (/** ... */) from nodes preceding a declaration.
pub fn extract_doc_comment(node: Node, source: &[u8]) -> Option<String> {
    let mut cursor = node;
    while let Some(prev) = cursor.prev_sibling() {
        if prev.kind() == node_kinds::BLOCK_COMMENT {
            let text = prev.utf8_text(source).ok()?;
            if text.starts_with("/**") {
                return Some(parse_javadoc(text));
            }
        } else if prev.kind() == node_kinds::MODIFIERS {
            cursor = prev;
            continue;
        }
        break;
    }
    None
}

fn parse_javadoc(text: &str) -> String {
    let content = text.trim_start_matches("/**").trim_end_matches("*/");
    content
        .lines()
        .map(|line| line.trim().trim_start_matches('*').trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Extracts the identifier name from a declaration node.
pub fn extract_identifier(node: Node, source: &[u8]) -> Option<String> {
    node.child_by_field_name("name")
        .and_then(|n| n.utf8_text(source).ok())
        .map(|s| s.to_string())
}

/// Determines visibility from modifiers node.
pub fn get_visibility(node: Node, source: &[u8]) -> (JavaVisibility, Visibility) {
    for child in node.children(&mut node.walk()) {
        if child.kind() == node_kinds::MODIFIERS {
            let text = child.utf8_text(source).unwrap_or("");
            if text.contains("public") {
                return (JavaVisibility::Public, Visibility::Public);
            } else if text.contains("protected") {
                return (JavaVisibility::Protected, Visibility::Public);
            } else if text.contains("private") {
                return (JavaVisibility::Private, Visibility::Private);
            }
        }
    }
    (JavaVisibility::PackagePrivate, Visibility::Private)
}

/// Converts a JavaItemType to the corresponding filter type.
pub fn to_filter_type(item_type: JavaItemType) -> FilterJavaItemType {
    match item_type {
        JavaItemType::Class => FilterJavaItemType::Class,
        JavaItemType::Interface => FilterJavaItemType::Interface,
        JavaItemType::Enum => FilterJavaItemType::Enum,
        JavaItemType::Record => FilterJavaItemType::Record,
        JavaItemType::Method => FilterJavaItemType::Method,
        JavaItemType::Field => FilterJavaItemType::Field,
        JavaItemType::Constructor => FilterJavaItemType::Constructor,
        JavaItemType::Annotation => FilterJavaItemType::Annotation,
    }
}
