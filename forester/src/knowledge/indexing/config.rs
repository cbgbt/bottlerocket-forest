//! Configuration loading for forester
//!
//! Handles parsing `.forester.toml` configuration files to control
//! indexing behavior, including file type filtering and Rust-specific options.

use path_clean::PathClean;
use serde::Deserialize;
use snafu::{ResultExt, Snafu};
use std::path::{Path, PathBuf};

use super::filter::{IndexingFilter, RustFilter, RustItemType};
use crate::knowledge::domain::{FileType, Visibility};

/// Forester configuration loaded from `.forester.toml`
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ForesterConfig {
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
}

impl ForesterConfig {
    /// Convert configuration to indexing filter
    pub fn to_indexing_filter(&self) -> Result<IndexingFilter, ForesterConfigError> {
        let rust_filter = if self.enabled_file_types.contains(&FileType::Rust) {
            Some(self.file_types.rust.to_rust_filter()?)
        } else {
            None
        };

        Ok(IndexingFilter::new(
            self.enabled_file_types.clone(),
            rust_filter,
        ))
    }
}

impl Default for ForesterConfig {
    fn default() -> Self {
        Self {
            enabled_file_types: default_file_types(),
            targets: Vec::new(),
            file_types: FileTypeConfig::default(),
        }
    }
}

/// File type specific configuration
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct FileTypeConfig {
    /// Rust-specific indexing controls
    #[serde(default)]
    pub rust: RustConfig,
}

/// Rust-specific indexing configuration
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct RustConfig {
    /// Visibility levels to index
    #[serde(default = "default_rust_visibility")]
    pub visibility: Vec<Visibility>,

    /// Item types to index ("all" or specific types)
    #[serde(default = "default_rust_items", deserialize_with = "deserialize_items")]
    items: Vec<RustItemType>,

    /// Minimum doc comment length in characters
    #[serde(default)]
    pub min_doc_length: usize,
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
    /// Convert to RustFilter
    fn to_rust_filter(&self) -> Result<RustFilter, ForesterConfigError> {
        Ok(RustFilter::new(
            self.visibility.clone(),
            self.items.clone(),
            self.min_doc_length,
        ))
    }
}

impl Default for RustConfig {
    fn default() -> Self {
        Self {
            visibility: default_rust_visibility(),
            items: default_rust_items(),
            min_doc_length: 0,
        }
    }
}

fn default_file_types() -> Vec<FileType> {
    vec![FileType::Markdown, FileType::Rust]
}

fn default_rust_visibility() -> Vec<Visibility> {
    vec![Visibility::Public]
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

/// Load forester configuration from `.forester.toml`
///
/// Returns `None` if the config file doesn't exist (not an error).
/// Returns an error if the file exists but is invalid.
pub fn load_forester_config(
    forest_root: impl AsRef<Path>,
) -> Result<Option<ForesterConfig>, ForesterConfigError> {
    use forester_config_error::*;

    let forest_root = forest_root.as_ref();
    let config_path = forest_root.join(".forester.toml");

    if !config_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&config_path).context(IoSnafu {
        path: config_path.display().to_string(),
    })?;

    let config: ForesterConfig = toml::from_str(&content).context(ParseSnafu)?;

    let forest_root_canonical = forest_root.canonicalize().context(IoSnafu {
        path: forest_root.display().to_string(),
    })?;

    for target in &config.targets {
        if target.is_absolute() {
            return Err(ForesterConfigError::InvalidPath {
                path: target.display().to_string(),
            });
        }

        let target_path = forest_root.join(target).clean();

        if let Ok(canonical) = target_path.canonicalize() {
            if !canonical.starts_with(&forest_root_canonical) {
                return Err(ForesterConfigError::PathEscapesRoot {
                    path: target.display().to_string(),
                });
            }
        } else if !target_path.starts_with(&forest_root_canonical) {
            return Err(ForesterConfigError::PathEscapesRoot {
                path: target.display().to_string(),
            });
        }
    }

    Ok(Some(config))
}

/// Errors that can occur when loading forester configuration
#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum ForesterConfigError {
    #[snafu(display("Failed to read config file: {path}"))]
    IoError {
        path: String,
        source: std::io::Error,
    },

    #[snafu(display("Failed to parse TOML config"))]
    ParseError { source: toml::de::Error },

    #[snafu(display("Invalid target path: {path}"))]
    InvalidPath { path: String },

    #[snafu(display("Target path escapes forest root: {path}"))]
    PathEscapesRoot { path: String },

    #[snafu(display("Invalid item type in configuration"))]
    InvalidItemType,
}

#[cfg(test)]
mod test {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_load_forester_config_with_valid_toml() {
        // Given A forest root with valid .forester.toml
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"
targets = ["docs", "kits/core-kit"]
"#;
        fs::write(temp_dir.path().join(".forester.toml"), config_content).unwrap();

        // When Loading the config
        let result = load_forester_config(temp_dir.path());

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
    fn test_load_forester_config_with_missing_file() {
        // Given A forest root without .forester.toml
        let temp_dir = TempDir::new().unwrap();

        // When Loading the config
        let result = load_forester_config(temp_dir.path());

        // Then It should return None (not an error)
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_load_forester_config_with_invalid_toml() {
        // Given A forest root with malformed TOML
        let temp_dir = TempDir::new().unwrap();
        let invalid_toml = "targets = [invalid syntax";
        fs::write(temp_dir.path().join(".forester.toml"), invalid_toml).unwrap();

        // When Loading the config
        let result = load_forester_config(temp_dir.path());

        // Then It should return ParseError
        assert!(matches!(
            result,
            Err(ForesterConfigError::ParseError { .. })
        ));
    }

    #[test]
    fn test_load_forester_config_with_empty_targets() {
        // Given A .forester.toml with empty targets array
        let temp_dir = TempDir::new().unwrap();
        let config_content = "targets = []";
        fs::write(temp_dir.path().join(".forester.toml"), config_content).unwrap();

        // When Loading the config
        let result = load_forester_config(temp_dir.path());

        // Then It should return config with empty targets
        assert!(result.is_ok());
        let config = result.unwrap().unwrap();
        assert!(config.targets.is_empty());
    }

    #[test]
    fn test_load_forester_config_validates_path_escaping() {
        // Given A .forester.toml with paths that escape forest root
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"
targets = ["../outside"]
"#;
        fs::write(temp_dir.path().join(".forester.toml"), config_content).unwrap();

        // When Loading the config
        let result = load_forester_config(temp_dir.path());

        // Then It should return PathEscapesRoot error
        assert!(matches!(
            result,
            Err(ForesterConfigError::PathEscapesRoot { .. })
        ));
    }

    #[test]
    fn test_load_forester_config_with_relative_paths() {
        // Given A .forester.toml with relative paths
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"
targets = ["docs", "bottlerocket", "kits/bottlerocket-core-kit"]
"#;
        fs::write(temp_dir.path().join(".forester.toml"), config_content).unwrap();

        // When Loading the config
        let result = load_forester_config(temp_dir.path());

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
    fn test_forester_config_with_file_type_filters() {
        // Given A .forester.toml with file type configuration
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"
enabled-file-types = ["markdown", "rust"]
targets = ["docs"]

[file-types.rust]
visibility = ["public", "crate"]
items = ["modules", "structs"]
min-doc-length = 30
"#;
        fs::write(temp_dir.path().join(".forester.toml"), config_content).unwrap();

        // When Loading the config
        let result = load_forester_config(temp_dir.path());

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
        assert_eq!(config.file_types.rust.min_doc_length, 30);
    }

    #[test]
    fn test_forester_config_to_indexing_filter() {
        // Given A config with Rust filtering
        let config = ForesterConfig {
            enabled_file_types: vec![FileType::Markdown, FileType::Rust],
            targets: vec![],
            file_types: FileTypeConfig {
                rust: RustConfig {
                    visibility: vec![Visibility::Public],
                    items: vec![RustItemType::Struct],
                    min_doc_length: 20,
                },
            },
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
            min_doc_length: 0,
        };

        // When Converting to filter
        let result = config.to_rust_filter();

        // Then It should expand to all item types
        assert!(result.is_ok());
    }
}
