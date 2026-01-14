//! Language-specific configuration types for indexing
//!
//! Contains configuration structs for Rust, Go, and Java source file indexing,
//! including visibility filters, item type selection, and documentation requirements.

use serde::Deserialize;

use super::CrumblyConfigError;
use crate::knowledge::domain::{DocLineCount, Visibility};
use crate::knowledge::indexing::filter::{
    GoFilter, GoItemType, JavaFilter, JavaItemType, RustFilter, RustItemType,
};

/// Configuration for indexing Rust source files
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct RustConfig {
    /// Visibility levels to index
    #[serde(default = "default_rust_visibility")]
    pub visibility: Vec<Visibility>,

    /// Item types to index ("all" or specific types)
    #[serde(
        default = "default_rust_items",
        deserialize_with = "deserialize_rust_items"
    )]
    pub items: Vec<RustItemType>,

    /// Minimum doc comment length in lines
    #[serde(default)]
    pub min_doc_lines: usize,
}

fn deserialize_rust_items<'de, D>(deserializer: D) -> Result<Vec<RustItemType>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;

    let strings: Vec<String> = Vec::deserialize(deserializer)?;

    if strings.len() == 1 && strings[0] == "all" {
        return Ok(default_rust_items());
    }

    strings
        .into_iter()
        .map(|s| match s.as_str() {
            "modules" => Ok(RustItemType::Module),
            "functions" => Ok(RustItemType::Function),
            "structs" => Ok(RustItemType::Struct),
            "enums" => Ok(RustItemType::Enum),
            "traits" => Ok(RustItemType::Trait),
            "impls" => Ok(RustItemType::Impl),
            "type-aliases" => Ok(RustItemType::TypeAlias),
            "constants" => Ok(RustItemType::Constant),
            _ => Err(D::Error::custom(format!("unknown item type: {}", s))),
        })
        .collect()
}

impl RustConfig {
    /// Convert to a RustFilter for use during indexing
    pub(super) fn to_rust_filter(&self) -> Result<RustFilter, CrumblyConfigError> {
        Ok(RustFilter::new(
            self.visibility.clone(),
            self.items.clone(),
            DocLineCount::new(self.min_doc_lines),
        ))
    }
}

impl Default for RustConfig {
    fn default() -> Self {
        Self {
            visibility: default_rust_visibility(),
            items: default_rust_items(),
            min_doc_lines: 0,
        }
    }
}

/// Configuration for indexing Go source files
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct GoConfig {
    /// Visibility levels to index
    #[serde(default = "default_visibility")]
    pub visibility: Vec<Visibility>,

    /// Item types to index
    #[serde(default = "default_go_items")]
    pub items: Vec<GoItemType>,

    /// Minimum doc comment length in lines
    #[serde(default)]
    pub min_doc_lines: usize,
}

impl GoConfig {
    /// Convert to a GoFilter for use during indexing
    pub(super) fn to_go_filter(&self) -> Result<GoFilter, CrumblyConfigError> {
        Ok(GoFilter::new(
            self.visibility.clone(),
            self.items.clone(),
            DocLineCount::new(self.min_doc_lines),
        ))
    }
}

impl Default for GoConfig {
    fn default() -> Self {
        Self {
            visibility: default_visibility(),
            items: default_go_items(),
            min_doc_lines: 0,
        }
    }
}

/// Configuration for indexing Java source files
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct JavaConfig {
    /// Visibility levels to index
    #[serde(default = "default_visibility")]
    pub visibility: Vec<Visibility>,

    /// Item types to index
    #[serde(default = "default_java_items")]
    pub items: Vec<JavaItemType>,

    /// Minimum doc comment length in lines
    #[serde(default)]
    pub min_doc_lines: usize,
}

impl JavaConfig {
    /// Convert to a JavaFilter for use during indexing
    pub(super) fn to_java_filter(&self) -> Result<JavaFilter, CrumblyConfigError> {
        Ok(JavaFilter::new(
            self.visibility.clone(),
            self.items.clone(),
            DocLineCount::new(self.min_doc_lines),
        ))
    }
}

impl Default for JavaConfig {
    fn default() -> Self {
        Self {
            visibility: default_visibility(),
            items: default_java_items(),
            min_doc_lines: 0,
        }
    }
}

fn default_rust_visibility() -> Vec<Visibility> {
    vec![Visibility::Public]
}

fn default_visibility() -> Vec<Visibility> {
    vec![Visibility::Public]
}

pub(super) fn default_go_items() -> Vec<GoItemType> {
    vec![
        GoItemType::Function,
        GoItemType::Method,
        GoItemType::Struct,
        GoItemType::Interface,
        GoItemType::Type,
        GoItemType::Const,
        GoItemType::Var,
    ]
}

pub(super) fn default_java_items() -> Vec<JavaItemType> {
    vec![
        JavaItemType::Class,
        JavaItemType::Interface,
        JavaItemType::Enum,
        JavaItemType::Record,
        JavaItemType::Method,
        JavaItemType::Field,
        JavaItemType::Constructor,
        JavaItemType::Annotation,
    ]
}

pub(super) fn default_rust_items() -> Vec<RustItemType> {
    vec![
        RustItemType::Module,
        RustItemType::Function,
        RustItemType::Struct,
        RustItemType::Enum,
        RustItemType::Trait,
        RustItemType::Impl,
        RustItemType::TypeAlias,
        RustItemType::Constant,
    ]
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_rust_config_handles_all_items() {
        let config = RustConfig {
            visibility: vec![Visibility::Public],
            items: default_rust_items(),
            min_doc_lines: 0,
        };
        let result = config.to_rust_filter();
        assert!(result.is_ok());
    }
}
