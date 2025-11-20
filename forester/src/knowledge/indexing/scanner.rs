//! File scanning for discovering indexable documentation

use bon::Builder;
use snafu::{ResultExt, Snafu};
use std::path::{Path, PathBuf};

use crate::knowledge::domain::{
    AbsolutePath, FileType, ForestRelativePath, RepoName, ScanConfig, Timestamp,
};

/// Scans directories for indexable files
#[derive(Debug)]
pub struct FileScanner {
    forest_root: PathBuf,
    config: ScanConfig,
}

impl FileScanner {
    /// Create a scanner for the given forest root directory
    pub fn new(forest_root: impl AsRef<Path>) -> Result<Self, ScanError> {
        Self::with_config(forest_root, ScanConfig::default())
    }

    /// Create a scanner with custom configuration
    pub fn with_config(
        forest_root: impl AsRef<Path>,
        config: ScanConfig,
    ) -> Result<Self, ScanError> {
        use scan_error::*;

        let forest_root = forest_root.as_ref();
        snafu::ensure!(
            forest_root.exists(),
            ForestRootNotFoundSnafu {
                path: forest_root.display().to_string()
            }
        );

        Ok(Self {
            forest_root: forest_root.to_path_buf(),
            config,
        })
    }

    /// Scan for all indexable files in the forest
    pub fn scan(&self) -> Result<Vec<IndexableFile>, ScanError> {
        self.scan_internal(None)
    }

    /// Scan a specific repository directory
    pub fn scan_repo(&self, repo_name: &RepoName) -> Result<Vec<IndexableFile>, ScanError> {
        self.scan_internal(Some(repo_name))
    }

    fn scan_internal(
        &self,
        filter_repo: Option<&RepoName>,
    ) -> Result<Vec<IndexableFile>, ScanError> {
        let mut files = Vec::new();

        let mut builder = ignore::WalkBuilder::new(&self.forest_root);
        builder
            .follow_links(false)
            .git_ignore(self.config.respect_gitignore)
            .filter_entry(|entry| {
                let file_name = entry.file_name().to_string_lossy();
                file_name != ".forester"
            });

        if self.config.use_foresterignore {
            builder.add_custom_ignore_filename(".foresterignore");
        }

        for result in builder.build() {
            use scan_error::*;

            let entry = result.context(WalkSnafu)?;

            let path = entry.path();

            if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                continue;
            }

            let file_type = FileType::from_path(path);
            if !file_type.is_indexable() {
                continue;
            }

            let relative = path.strip_prefix(&self.forest_root).map_err(|_| {
                ScanError::InvalidPathStructure {
                    path: path.display().to_string(),
                }
            })?;

            let repo_name = relative
                .components()
                .next()
                .and_then(|c| c.as_os_str().to_str())
                .ok_or_else(|| ScanError::InvalidPathStructure {
                    path: path.display().to_string(),
                })?;

            let repo_name = RepoName::try_new(repo_name).map_err(|e| ScanError::PathCreation {
                source: Box::new(e) as Box<dyn std::error::Error + Send + Sync>,
            })?;

            if let Some(filter) = filter_repo
                && &repo_name != filter
            {
                continue;
            }

            let metadata = std::fs::metadata(path).map_err(|e| ScanError::IoError {
                path: path.display().to_string(),
                source: e,
            })?;

            let last_modified = metadata
                .modified()
                .map_err(|e| ScanError::IoError {
                    path: path.display().to_string(),
                    source: e,
                })?
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| ScanError::IoError {
                    path: path.display().to_string(),
                    source: std::io::Error::other("invalid modification time"),
                })?
                .as_secs() as i64;

            let absolute_path = AbsolutePath::try_new(path.display().to_string()).map_err(|e| {
                ScanError::PathCreation {
                    source: Box::new(e) as Box<dyn std::error::Error + Send + Sync>,
                }
            })?;

            let relative_path = ForestRelativePath::try_new(relative.display().to_string())
                .map_err(|e| ScanError::PathCreation {
                    source: Box::new(e) as Box<dyn std::error::Error + Send + Sync>,
                })?;

            files.push(
                IndexableFile::builder()
                    .absolute_path(absolute_path)
                    .relative_path(relative_path)
                    .repo_name(repo_name)
                    .file_type(file_type)
                    .last_modified(Timestamp::from_secs(last_modified))
                    .build(),
            );
        }

        Ok(files)
    }
}

/// A file that can be indexed
#[derive(Debug, Clone, PartialEq, Eq, Builder)]
#[builder(on(_, into))]
#[non_exhaustive]
pub struct IndexableFile {
    pub absolute_path: AbsolutePath,
    pub relative_path: ForestRelativePath,
    pub repo_name: RepoName,
    pub file_type: FileType,
    pub last_modified: Timestamp,
}

/// Errors that can occur during file scanning
#[derive(Debug, Snafu, miette::Diagnostic)]
#[snafu(module)]
pub enum ScanError {
    #[snafu(display("Forest root not found: {path}"))]
    #[diagnostic(
        code(forester::scanner::forest_root_not_found),
        help("Ensure the path exists and is accessible")
    )]
    ForestRootNotFound { path: String },

    #[snafu(display("Error walking directory tree"))]
    #[diagnostic(
        code(forester::scanner::walk_error),
        help("Check file permissions and filesystem health")
    )]
    Walk { source: ignore::Error },

    #[snafu(display("Permission denied: {path}"))]
    #[diagnostic(
        code(forester::scanner::permission_denied),
        help("Check file and directory permissions")
    )]
    PermissionDenied { path: String },

    #[snafu(display("I/O error scanning {path}"))]
    #[diagnostic(
        code(forester::scanner::io_error),
        help("Check that the path is accessible and the filesystem is healthy")
    )]
    IoError {
        path: String,
        source: std::io::Error,
    },

    #[snafu(display("Invalid path structure: {path}"))]
    #[diagnostic(
        code(forester::scanner::invalid_path_structure),
        help("Ensure the path follows the expected forest directory structure")
    )]
    InvalidPathStructure { path: String },

    #[snafu(display("Failed to create path type"))]
    #[diagnostic(
        code(forester::scanner::path_creation_failed),
        help("The path may contain invalid characters or exceed length limits")
    )]
    PathCreation {
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

#[cfg(test)]
mod test {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn scanner_no_git(path: impl AsRef<Path>) -> FileScanner {
        let config = ScanConfig::builder()
            .respect_gitignore(false)
            .use_foresterignore(false)
            .build();
        FileScanner::with_config(path, config).unwrap()
    }

    #[test]
    fn test_scanner_rejects_nonexistent_root() {
        // Given A nonexistent directory
        let nonexistent = Path::new("/nonexistent/path/to/forest");

        // When Creating a scanner
        let result = FileScanner::new(nonexistent);

        // Then It should fail with ForestRootNotFound
        assert!(matches!(result, Err(ScanError::ForestRootNotFound { .. })));
    }

    #[test]
    fn test_scan_finds_markdown_files() {
        // Given A forest with markdown files
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("bottlerocket");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("README.md"), "# Test").unwrap();
        fs::write(repo_dir.join("DESIGN.md"), "# Design").unwrap();

        // When Scanning
        let scanner = FileScanner::new(temp_dir.path()).unwrap();
        let files = scanner.scan().unwrap();

        // Then It should find markdown files
        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|f| f.file_type == FileType::Markdown));
    }

    #[test]
    fn test_scan_finds_rust_files() {
        // Given A forest with rust files
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("twoliter");
        fs::create_dir_all(repo_dir.join("src")).unwrap();
        fs::write(repo_dir.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(repo_dir.join("src/lib.rs"), "pub fn test() {}").unwrap();

        // When Scanning without gitignore
        let scanner = scanner_no_git(temp_dir.path());
        let files = scanner.scan().unwrap();

        // Then It should find rust files
        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|f| f.file_type == FileType::Rust));
    }

    #[test]
    fn test_scan_ignores_unsupported_files() {
        // Given A forest with mixed file types
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("bottlerocket");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("README.md"), "# Test").unwrap();
        fs::write(repo_dir.join("Cargo.toml"), "[package]").unwrap();
        fs::write(repo_dir.join("LICENSE"), "MIT").unwrap();
        fs::write(repo_dir.join("data.json"), "{}").unwrap();

        // When Scanning
        let scanner = FileScanner::new(temp_dir.path()).unwrap();
        let files = scanner.scan().unwrap();

        // Then It should only return .md and .rs files
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].file_type, FileType::Markdown);
    }

    #[test]
    fn test_scan_extracts_repo_name() {
        // Given A forest with repo structure
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("bottlerocket");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("README.md"), "# Test").unwrap();

        // When Scanning
        let scanner = FileScanner::new(temp_dir.path()).unwrap();
        let files = scanner.scan().unwrap();

        // Then Repo name should be extracted correctly
        assert_eq!(files.len(), 1);
        assert_eq!(
            files[0].repo_name,
            RepoName::try_new("bottlerocket").unwrap()
        );
    }

    #[test]
    fn test_scan_captures_last_modified() {
        // Given A file with known modification time
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        let file_path = repo_dir.join("test.md");
        fs::write(&file_path, "content").unwrap();
        let metadata = fs::metadata(&file_path).unwrap();
        let expected_mtime = metadata
            .modified()
            .unwrap()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // When Scanning
        let scanner = FileScanner::new(temp_dir.path()).unwrap();
        let files = scanner.scan().unwrap();

        // Then Last modified should match file metadata
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].last_modified.as_secs(), expected_mtime);
    }

    #[test]
    fn test_scan_repo_filters_by_repo() {
        // Given A forest with multiple repos
        let temp_dir = TempDir::new().unwrap();
        let bottlerocket_dir = temp_dir.path().join("bottlerocket");
        let twoliter_dir = temp_dir.path().join("twoliter");
        fs::create_dir(&bottlerocket_dir).unwrap();
        fs::create_dir(&twoliter_dir).unwrap();
        fs::write(bottlerocket_dir.join("README.md"), "# BR").unwrap();
        fs::write(twoliter_dir.join("README.md"), "# TL").unwrap();

        // When Scanning specific repo
        let scanner = FileScanner::new(temp_dir.path()).unwrap();
        let repo = RepoName::try_new("bottlerocket").unwrap();
        let files = scanner.scan_repo(&repo).unwrap();

        // Then Only files from that repo should be returned
        assert_eq!(files.len(), 1);
        assert!(
            files
                .iter()
                .all(|f| f.repo_name == RepoName::try_new("bottlerocket").unwrap())
        );
    }

    #[test]
    fn test_scan_handles_nested_directories() {
        // Given A forest with nested directory structure
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("bottlerocket");
        fs::create_dir_all(repo_dir.join("docs/architecture")).unwrap();
        fs::write(repo_dir.join("docs/README.md"), "# Docs").unwrap();
        fs::write(repo_dir.join("docs/architecture/boot.md"), "# Boot").unwrap();

        // When Scanning
        let scanner = FileScanner::new(temp_dir.path()).unwrap();
        let files = scanner.scan().unwrap();

        // Then It should find files in nested directories
        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|f| f.file_type == FileType::Markdown));
    }

    #[test]
    fn test_scan_preserves_relative_paths() {
        // Given A forest with files in subdirectories
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("bottlerocket");
        fs::create_dir_all(repo_dir.join("docs")).unwrap();
        fs::write(repo_dir.join("docs/guide.md"), "# Guide").unwrap();

        // When Scanning
        let scanner = FileScanner::new(temp_dir.path()).unwrap();
        let files = scanner.scan().unwrap();

        // Then Relative path should be preserved
        assert_eq!(files.len(), 1);
        assert_eq!(
            files[0].relative_path,
            ForestRelativePath::try_new("bottlerocket/docs/guide.md").unwrap()
        );
    }

    #[test]
    fn test_always_ignores_forester_directory() {
        // Given A forest with .forester directory containing indexable files
        let temp_dir = TempDir::new().unwrap();
        let forester_dir = temp_dir.path().join(".forester");
        fs::create_dir(&forester_dir).unwrap();
        fs::write(forester_dir.join("index.db"), "data").unwrap();
        fs::write(forester_dir.join("notes.md"), "# Notes").unwrap();

        // When Scanning
        let scanner = FileScanner::new(temp_dir.path()).unwrap();
        let files = scanner.scan().unwrap();

        // Then .forester directory should never be scanned
        assert!(files.is_empty());
    }

    #[test]
    fn test_scan_config_default_values() {
        // Given Default ScanConfig
        let config = ScanConfig::default();

        // Then It should have expected defaults
        assert!(config.respect_gitignore);
        assert!(config.use_foresterignore);
    }

    #[test]
    fn test_scanner_with_custom_config() {
        // Given A custom ScanConfig
        let config = ScanConfig::builder()
            .respect_gitignore(false)
            .use_foresterignore(false)
            .build();

        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("test.md"), "# Test").unwrap();

        // When Creating scanner with custom config
        let scanner = FileScanner::with_config(temp_dir.path(), config).unwrap();
        let files = scanner.scan().unwrap();

        // Then Scanner should be created successfully
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn test_gitignore_respected_by_default() {
        // Given A forest with .gitignore in a git repo
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("repo");
        fs::create_dir(&repo_dir).unwrap();

        // Create .git directory (required for .gitignore to work)
        fs::create_dir(repo_dir.join(".git")).unwrap();

        fs::write(repo_dir.join(".gitignore"), "ignored.md\n").unwrap();
        fs::write(repo_dir.join("included.md"), "# Included").unwrap();
        fs::write(repo_dir.join("ignored.md"), "# Ignored").unwrap();

        // When Scanning with default config
        let scanner = FileScanner::new(temp_dir.path()).unwrap();
        let files = scanner.scan().unwrap();

        // Then Gitignored file should not be found (only included.md)
        assert_eq!(files.len(), 1);
        assert!(files[0].relative_path.to_string().contains("included.md"));
    }

    #[test]
    fn test_gitignore_can_be_disabled() {
        // Given A forest with .gitignore
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join(".gitignore"), "ignored.md\n").unwrap();
        fs::write(repo_dir.join("included.md"), "# Included").unwrap();
        fs::write(repo_dir.join("ignored.md"), "# Ignored").unwrap();

        // When Scanning with gitignore disabled
        let config = ScanConfig::builder().respect_gitignore(false).build();
        let scanner = FileScanner::with_config(temp_dir.path(), config).unwrap();
        let files = scanner.scan().unwrap();

        // Then Both files should be found
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_foresterignore_excludes_files() {
        // Given A forest with .foresterignore
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join(".foresterignore"), "excluded/\n").unwrap();
        let repo_dir = temp_dir.path().join("repo");
        fs::create_dir_all(repo_dir.join("excluded")).unwrap();
        fs::create_dir_all(repo_dir.join("included")).unwrap();
        fs::write(repo_dir.join("excluded/doc.md"), "# Excluded").unwrap();
        fs::write(repo_dir.join("included/doc.md"), "# Included").unwrap();

        // When Scanning
        let scanner = FileScanner::new(temp_dir.path()).unwrap();
        let files = scanner.scan().unwrap();

        // Then Only included file should be found
        assert_eq!(files.len(), 1);
        assert!(files[0].relative_path.to_string().contains("included"));
    }

    #[test]
    fn test_foresterignore_can_be_disabled() {
        // Given A forest with .foresterignore
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join(".foresterignore"), "excluded/\n").unwrap();
        let repo_dir = temp_dir.path().join("repo");
        fs::create_dir_all(repo_dir.join("excluded")).unwrap();
        fs::write(repo_dir.join("excluded/doc.md"), "# Excluded").unwrap();

        // When Scanning with foresterignore disabled
        let config = ScanConfig::builder().use_foresterignore(false).build();
        let scanner = FileScanner::with_config(temp_dir.path(), config).unwrap();
        let files = scanner.scan().unwrap();

        // Then File should be found
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn test_foresterignore_with_subdirectories() {
        // Given A forest with .foresterignore excluding a subdirectory
        let temp_dir = TempDir::new().unwrap();

        // Create .git directory to prevent global gitignore interference
        fs::create_dir(temp_dir.path().join(".git")).unwrap();

        fs::write(temp_dir.path().join(".foresterignore"), "*/vendor/\n").unwrap();
        let repo_dir = temp_dir.path().join("repo");
        fs::create_dir_all(repo_dir.join("vendor")).unwrap();
        fs::create_dir_all(repo_dir.join("docs")).unwrap();
        fs::write(repo_dir.join("vendor/doc.md"), "# Vendor").unwrap();
        fs::write(repo_dir.join("docs/doc.md"), "# Docs").unwrap();

        // When Scanning
        let scanner = FileScanner::new(temp_dir.path()).unwrap();
        let files = scanner.scan().unwrap();

        // Then Only docs file should be found (vendor excluded)
        assert_eq!(files.len(), 1);
        assert!(files[0].relative_path.to_string().contains("docs"));
    }

    #[test]
    fn test_foresterignore_wildcard_patterns() {
        // Given A forest with .foresterignore using wildcards
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join(".foresterignore"), "*.tmp.md\n").unwrap();
        let repo_dir = temp_dir.path().join("repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("keep.md"), "# Keep").unwrap();
        fs::write(repo_dir.join("ignore.tmp.md"), "# Ignore").unwrap();

        // When Scanning
        let scanner = FileScanner::new(temp_dir.path()).unwrap();
        let files = scanner.scan().unwrap();

        // Then Only non-matching file should be found
        assert_eq!(files.len(), 1);
        assert!(files[0].relative_path.to_string().contains("keep.md"));
    }
}
