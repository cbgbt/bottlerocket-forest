//! Configuration loading for forester
//!
//! Handles parsing `.forester.toml` configuration files to control
//! indexing behavior, particularly multi-target scanning.

use path_clean::PathClean;
use serde::Deserialize;
use snafu::{ResultExt, Snafu};
use std::path::{Path, PathBuf};

/// Forester configuration loaded from `.forester.toml`
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ForesterConfig {
    /// Scan targets relative to forest root
    ///
    /// Each target is scanned independently with its own gitignore context.
    /// Empty list means scan from forest root.
    #[serde(default)]
    pub targets: Vec<PathBuf>,
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
}
