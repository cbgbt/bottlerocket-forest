//! Configuration loading and parsing for crumblying
//!
//! Loads and validates `crumbly.toml` configuration files that control indexing
//! behavior. Configuration includes file type filtering, Rust-specific options,
//! scan targets, and search result boosting rules.

use path_clean::PathClean;
use serde::Deserialize;
use snafu::{ResultExt, Snafu};
use std::path::{Path, PathBuf};

use super::filter::{GoFilter, GoItemType, IndexingFilter, JavaFilter, JavaItemType, RustFilter, RustItemType};
use crate::knowledge::constants::SEMBLY_CONFIG;
use crate::knowledge::domain::{DocLineCount, FileType, Visibility};
use crate::knowledge::scoring::BoostRule;

/// Configuration loaded from `crumbly.toml` in the forest root
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct CrumblyConfig {
    /// File types to index
    #[serde(default = "default_file_types")]
    pub enabled_file_types: Vec<FileType>,

    /// Scan targets relative to forest root
    ///
    /// Each target is scanned independently with its own gitignore context.
    /// Empty list means scan from forest root.
    #[serde(default)]
    pub targets: Vec<PathBuf>,

    /// File type specific configuration
    #[serde(default)]
    pub file_types: FileTypeConfig,

    /// Search result score boosting rules
    #[serde(default)]
    pub boost_rules: Vec<BoostRule>,
}

impl CrumblyConfig {
    /// Convert configuration to an IndexingFilter for use during scanning
    pub fn to_indexing_filter(&self) -> Result<IndexingFilter, CrumblyConfigError> {
        let rust_filter = if self.enabled_file_types.contains(&FileType::Rust) {
            Some(self.file_types.rust.to_rust_filter()?)
        } else {
            None
        };

        let go_filter = if self.enabled_file_types.contains(&FileType::Go) {
            Some(self.file_types.go.to_go_filter()?)
        } else {
            None
        };

        let java_filter = if self.enabled_file_types.contains(&FileType::Java) {
            Some(self.file_types.java.to_java_filter()?)
        } else {
            None
        };

        Ok(IndexingFilter::new(
            self.enabled_file_types.clone(),
            rust_filter,
            go_filter,
            java_filter,
        ))
    }
}

impl Default for CrumblyConfig {
    fn default() -> Self {
        Self {
            enabled_file_types: default_file_types(),
            targets: Vec::new(),
            file_types: FileTypeConfig::default(),
            boost_rules: Vec::new(),
        }
    }
}

/// Configuration options specific to different file types
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct FileTypeConfig {
    /// Rust-specific indexing controls
    #[serde(default)]
    pub rust: RustConfig,

    /// Go-specific indexing controls
    #[serde(default)]
    pub go: GoConfig,

    /// Java-specific indexing controls
    #[serde(default)]
    pub java: JavaConfig,
}

/// Configuration for indexing Rust source files
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct RustConfig {
    /// Visibility levels to index
    #[serde(default = "default_rust_visibility")]
    pub visibility: Vec<Visibility>,

    /// Item types to index ("all" or specific types)
    #[serde(default = "default_rust_items", deserialize_with = "deserialize_items")]
    pub items: Vec<RustItemType>,

    /// Minimum doc comment length in lines
    #[serde(default)]
    pub min_doc_lines: usize,
}

fn deserialize_items<'de, D>(deserializer: D) -> Result<Vec<RustItemType>, D::Error>
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
    fn to_rust_filter(&self) -> Result<RustFilter, CrumblyConfigError> {
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
    fn to_go_filter(&self) -> Result<GoFilter, CrumblyConfigError> {
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
    fn to_java_filter(&self) -> Result<JavaFilter, CrumblyConfigError> {
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

fn default_file_types() -> Vec<FileType> {
    vec![FileType::Markdown, FileType::Rust]
}

fn default_rust_visibility() -> Vec<Visibility> {
    vec![Visibility::Public]
}

fn default_visibility() -> Vec<Visibility> {
    vec![Visibility::Public]
}

fn default_go_items() -> Vec<GoItemType> {
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

fn default_java_items() -> Vec<JavaItemType> {
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

fn default_rust_items() -> Vec<RustItemType> {
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

/// Load and validate crumbly configuration from `crumbly.toml`
///
/// Returns `None` if the config file doesn't exist. Returns an error if the file
/// exists but contains invalid TOML or violates validation rules (e.g., paths
/// that escape the forest root).
pub fn load_crumbly_config(
    index_root: impl AsRef<Path>,
) -> Result<Option<CrumblyConfig>, CrumblyConfigError> {
    use crumbly_config_error::*;

    let index_root = index_root.as_ref();
    let config_path = index_root.join(SEMBLY_CONFIG);

    if !config_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&config_path).context(IoSnafu {
        path: config_path.display().to_string(),
    })?;

    let config: CrumblyConfig = toml::from_str(&content).context(ParseSnafu)?;

    let index_root_canonical = index_root.canonicalize().context(IoSnafu {
        path: index_root.display().to_string(),
    })?;

    // Best-effort validation to catch user errors in configuration.
    // TOCTOU: Paths could change between validation and use, but this is not a
    // security boundary - it's a developer tool running with user's permissions.
    for target in &config.targets {
        if target.is_absolute() {
            return Err(CrumblyConfigError::InvalidPath {
                path: target.display().to_string(),
            });
        }

        let target_path = index_root.join(target).clean();

        if let Ok(canonical) = target_path.canonicalize() {
            if !canonical.starts_with(&index_root_canonical) {
                return Err(CrumblyConfigError::PathEscapesRoot {
                    path: target.display().to_string(),
                });
            }
        } else if !target_path.starts_with(&index_root_canonical) {
            return Err(CrumblyConfigError::PathEscapesRoot {
                path: target.display().to_string(),
            });
        }
    }

    Ok(Some(config))
}

/// Errors that can occur when loading crumbly configuration.
#[derive(Debug, Snafu)]
#[snafu(module)]
#[allow(missing_docs)]
#[non_exhaustive]
pub enum CrumblyConfigError {
    /// Failed to read the configuration file.
    #[snafu(display("Failed to read config file: {path}"))]
    IoError {
        path: String,
        source: std::io::Error,
    },
    /// Configuration file contains invalid TOML.
    #[snafu(display("Failed to parse TOML config"))]
    ParseError { source: toml::de::Error },
    /// Target path is invalid (e.g., absolute path).
    #[snafu(display("Invalid target path: {path}"))]
    InvalidPath { path: String },
    /// Target path would escape the forest root directory.
    #[snafu(display("Target path escapes forest root: {path}"))]
    PathEscapesRoot { path: String },
    /// Unrecognized item type in configuration.
    #[snafu(display("Invalid item type in configuration"))]
    InvalidItemType,
}

#[cfg(test)]
mod test {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_load_crumbly_config_with_valid_toml() {
        // Given A forest root with valid crumbly.toml
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"
targets = ["docs", "kits/core-kit"]
"#;
        fs::write(temp_dir.path().join("crumbly.toml"), config_content).unwrap();

        // When Loading the config
        let result = load_crumbly_config(temp_dir.path());

        // Then It should return the parsed config
        assert!(result.is_ok());
        let config = result.unwrap();
        assert!(config.is_some());
        let config = config.unwrap();
        assert_eq!(config.targets.len(), 2);
        assert_eq!(config.targets[0], PathBuf::from("docs"));
        assert_eq!(config.targets[1], PathBuf::from("kits/core-kit"));
    }

    #[test]
    fn test_load_crumbly_config_with_missing_file() {
        // Given A forest root without crumbly.toml
        let temp_dir = TempDir::new().unwrap();

        // When Loading the config
        let result = load_crumbly_config(temp_dir.path());

        // Then It should return None (not an error)
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_load_crumbly_config_with_invalid_toml() {
        // Given A forest root with malformed TOML
        let temp_dir = TempDir::new().unwrap();
        let invalid_toml = "targets = [invalid syntax";
        fs::write(temp_dir.path().join("crumbly.toml"), invalid_toml).unwrap();

        // When Loading the config
        let result = load_crumbly_config(temp_dir.path());

        // Then It should return ParseError
        assert!(matches!(result, Err(CrumblyConfigError::ParseError { .. })));
    }

    #[test]
    fn test_load_crumbly_config_with_empty_targets() {
        // Given A crumbly.toml with empty targets array
        let temp_dir = TempDir::new().unwrap();
        let config_content = "targets = []";
        fs::write(temp_dir.path().join("crumbly.toml"), config_content).unwrap();

        // When Loading the config
        let result = load_crumbly_config(temp_dir.path());

        // Then It should return config with empty targets
        assert!(result.is_ok());
        let config = result.unwrap().unwrap();
        assert!(config.targets.is_empty());
    }

    #[test]
    fn test_load_crumbly_config_validates_path_escaping() {
        // Given A crumbly.toml with paths that escape forest root
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"
targets = ["../outside"]
"#;
        fs::write(temp_dir.path().join("crumbly.toml"), config_content).unwrap();

        // When Loading the config
        let result = load_crumbly_config(temp_dir.path());

        // Then It should return PathEscapesRoot error
        assert!(matches!(
            result,
            Err(CrumblyConfigError::PathEscapesRoot { .. })
        ));
    }

    #[test]
    fn test_load_crumbly_config_with_relative_paths() {
        // Given A crumbly.toml with relative paths
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"
targets = ["docs", "bottlerocket", "kits/bottlerocket-core-kit"]
"#;
        fs::write(temp_dir.path().join("crumbly.toml"), config_content).unwrap();

        // When Loading the config
        let result = load_crumbly_config(temp_dir.path());

        // Then It should parse successfully
        assert!(result.is_ok());
        let config = result.unwrap().unwrap();
        assert_eq!(config.targets.len(), 3);
        assert_eq!(config.targets[0], PathBuf::from("docs"));
        assert_eq!(config.targets[1], PathBuf::from("bottlerocket"));
        assert_eq!(
            config.targets[2],
            PathBuf::from("kits/bottlerocket-core-kit")
        );
    }

    #[test]
    fn test_crumbly_config_with_file_type_filters() {
        // Given A crumbly.toml with file type configuration
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"
enabled-file-types = ["markdown", "rust"]
targets = ["docs"]

[file-types.rust]
visibility = ["public", "crate"]
items = ["modules", "structs"]
min-doc-lines = 30
"#;
        fs::write(temp_dir.path().join("crumbly.toml"), config_content).unwrap();

        // When Loading the config
        let result = load_crumbly_config(temp_dir.path());

        // Then It should parse successfully
        assert!(result.is_ok());
        let config = result.unwrap().unwrap();
        assert_eq!(
            config.enabled_file_types,
            vec![FileType::Markdown, FileType::Rust]
        );
        assert_eq!(
            config.file_types.rust.visibility,
            vec![Visibility::Public, Visibility::Crate]
        );
        assert_eq!(config.file_types.rust.min_doc_lines, 30);
    }

    #[test]
    fn test_crumbly_config_to_indexing_filter() {
        // Given A config with Rust filtering
        let config = CrumblyConfig {
            enabled_file_types: vec![FileType::Markdown, FileType::Rust],
            targets: vec![],
            file_types: FileTypeConfig {
                rust: RustConfig {
                    visibility: vec![Visibility::Public],
                    items: vec![RustItemType::Struct],
                    min_doc_lines: 20,
                },
                go: GoConfig::default(),
            },
            boost_rules: vec![],
        };

        // When Converting to indexing filter
        let result = config.to_indexing_filter();

        // Then It should create a valid filter
        assert!(result.is_ok());
        let filter = result.unwrap();
        assert!(filter.should_index_file_type(FileType::Markdown));
        assert!(filter.should_index_file_type(FileType::Rust));
        assert!(filter.rust_filter().is_some());
    }

    #[test]
    fn test_rust_config_handles_all_items() {
        // Given A config with "all" items
        let config = RustConfig {
            visibility: vec![Visibility::Public],
            items: default_rust_items(),
            min_doc_lines: 0,
        };

        // When Converting to filter
        let result = config.to_rust_filter();

        // Then It should expand to all item types
        assert!(result.is_ok());
    }

    #[test]
    fn test_load_crumbly_config_with_boost_rules() {
        // Given A crumbly.toml with boost rules
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"
targets = ["docs"]

[[boost-rules]]
pattern = "**/README.md"
multiplier = 1.5

[[boost-rules]]
pattern = "**/*.md"
multiplier = 1.2
"#;
        fs::write(temp_dir.path().join("crumbly.toml"), config_content).unwrap();

        // When Loading the config
        let result = load_crumbly_config(temp_dir.path());

        // Then It should parse boost rules successfully
        assert!(result.is_ok());
        let config = result.unwrap().unwrap();
        assert_eq!(config.boost_rules.len(), 2);
        assert_eq!(config.boost_rules[0].multiplier.into_inner(), 1.5);
        assert_eq!(config.boost_rules[1].multiplier.into_inner(), 1.2);
    }

    #[test]
    fn test_load_crumbly_config_with_boost_rules_and_descriptions() {
        // Given A crumbly.toml with boost rules including descriptions
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"
[[boost-rules]]
description = "README files"
pattern = "**/README.md"
multiplier = 1.3
"#;
        fs::write(temp_dir.path().join("crumbly.toml"), config_content).unwrap();

        // When Loading the config
        let result = load_crumbly_config(temp_dir.path());

        // Then It should parse with description
        assert!(result.is_ok());
        let config = result.unwrap().unwrap();
        assert_eq!(config.boost_rules.len(), 1);
        assert_eq!(config.boost_rules[0].description, "README files");
    }

    #[test]
    fn test_load_crumbly_config_boost_rules_optional_description() {
        // Given A crumbly.toml with boost rules without descriptions
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"
[[boost-rules]]
pattern = "docs/**"
multiplier = 1.1
"#;
        fs::write(temp_dir.path().join("crumbly.toml"), config_content).unwrap();

        // When Loading the config
        let result = load_crumbly_config(temp_dir.path());

        // Then It should parse with empty description
        assert!(result.is_ok());
        let config = result.unwrap().unwrap();
        assert_eq!(config.boost_rules.len(), 1);
        assert_eq!(config.boost_rules[0].description, "");
    }
}
