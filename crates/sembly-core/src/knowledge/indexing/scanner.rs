//! File discovery for indexable documentation in the forest
//!
//! The [`FileScanner`] walks the forest directory structure to discover files
//! that can be indexed (.md and .rs files). It respects .gitignore and
//! .semblyignore patterns, and can be configured to scan specific target
//! directories or the entire forest.

use bon::Builder;
use snafu::{ResultExt, Snafu};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::{IndexingFilter, ProgressReporter};
use crate::knowledge::constants::{SEMBLY_DIR, SEMBLY_IGNORE};
use crate::knowledge::domain::{
    AbsolutePath, FileType, ForestRelativePath, RepoName, ScanConfig, Timestamp,
};

/// Discovers indexable files in the forest directory structure
pub struct FileScanner {
    forest_root: PathBuf,
    config: ScanConfig,
    filter: IndexingFilter,
    progress: Option<Arc<dyn ProgressReporter>>,
}

impl std::fmt::Debug for FileScanner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileScanner")
            .field("forest_root", &self.forest_root)
            .field("config", &self.config)
            .field("filter", &self.filter)
            .field(
                "progress",
                &self.progress.as_ref().map(|_| "Some(ProgressReporter)"),
            )
            .finish()
    }
}

impl FileScanner {
    /// Create a scanner for the given forest root directory
    pub fn new(forest_root: impl AsRef<Path>) -> Result<Self, ScanError> {
        Self::with_progress(
            forest_root,
            ScanConfig::default(),
            IndexingFilter::default(),
            None,
        )
    }

    /// Create a scanner with custom configuration
    pub fn with_config(
        forest_root: impl AsRef<Path>,
        config: ScanConfig,
    ) -> Result<Self, ScanError> {
        Self::with_progress(forest_root, config, IndexingFilter::default(), None)
    }

    /// Create a scanner with custom configuration and filter
    pub fn with_config_and_filter(
        forest_root: impl AsRef<Path>,
        config: ScanConfig,
        filter: IndexingFilter,
    ) -> Result<Self, ScanError> {
        Self::with_progress(forest_root, config, filter, None)
    }

    /// Create a scanner with optional progress reporting
    pub fn with_progress(
        forest_root: impl AsRef<Path>,
        config: ScanConfig,
        filter: IndexingFilter,
        progress: Option<Arc<dyn ProgressReporter>>,
    ) -> Result<Self, ScanError> {
        use scan_error::*;

        let forest_root = forest_root.as_ref();
        snafu::ensure!(
            forest_root.exists(),
            ForestRootNotFoundSnafu {
                path: forest_root.display().to_string()
            }
        );
        snafu::ensure!(
            forest_root.is_dir(),
            ForestRootNotDirectorySnafu {
                path: forest_root.display().to_string()
            }
        );

        Ok(Self {
            forest_root: forest_root.to_path_buf(),
            config,
            filter,
            progress,
        })
    }

    /// Scan for all indexable files in the forest
    pub fn scan(&self) -> Result<Vec<IndexableFile>, ScanError> {
        if let Some(progress) = &self.progress {
            progress.scanning_started();
        }

        let result = self.scan_internal(None);

        if result.is_ok()
            && let Some(progress) = &self.progress
        {
            progress.scanning_completed();
        }

        result
    }

    /// Scan a specific repository directory
    pub fn scan_repo(&self, repo_name: &RepoName) -> Result<Vec<IndexableFile>, ScanError> {
        if let Some(progress) = &self.progress {
            progress.scanning_started();
        }

        let result = self.scan_internal(Some(repo_name));

        if result.is_ok()
            && let Some(progress) = &self.progress
        {
            progress.scanning_completed();
        }

        result
    }

    fn scan_internal(
        &self,
        filter_repo: Option<&RepoName>,
    ) -> Result<Vec<IndexableFile>, ScanError> {
        if self.config.targets.is_empty() {
            self.scan_from_root(filter_repo)
        } else {
            let mut all_files = Vec::new();
            for target in &self.config.targets {
                let target_path = self.forest_root.join(target);
                if !target_path.exists() {
                    continue;
                }
                let files = self.scan_target(&target_path, filter_repo)?;
                all_files.extend(files);
            }
            Ok(all_files)
        }
    }

    fn scan_from_root(
        &self,
        filter_repo: Option<&RepoName>,
    ) -> Result<Vec<IndexableFile>, ScanError> {
        self.scan_with_builder(&self.forest_root, filter_repo)
    }

    fn scan_target(
        &self,
        target_path: &Path,
        filter_repo: Option<&RepoName>,
    ) -> Result<Vec<IndexableFile>, ScanError> {
        self.scan_with_builder(target_path, filter_repo)
    }

    fn scan_with_builder(
        &self,
        root: &Path,
        filter_repo: Option<&RepoName>,
    ) -> Result<Vec<IndexableFile>, ScanError> {
        let mut files = Vec::new();

        let mut builder = ignore::WalkBuilder::new(root);
        builder
            .follow_links(false)
            .git_ignore(self.config.respect_gitignore)
            .filter_entry(|entry| {
                let file_name = entry.file_name().to_string_lossy();
                file_name != SEMBLY_DIR
            });

        if self.config.use_semblyignore {
            builder.add_custom_ignore_filename(SEMBLY_IGNORE);
        }

        for result in builder.build() {
            use scan_error::*;

            let entry = result.context(WalkSnafu)?;

            let path = entry.path();

            if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                continue;
            }

            let file_type = FileType::from_path(path);

            if !self.filter.should_index_file_type(file_type) {
                continue;
            }

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

            let indexable_file = IndexableFile::builder()
                .absolute_path(absolute_path)
                .relative_path(relative_path)
                .repo_name(repo_name)
                .file_type(file_type)
                .last_modified(Timestamp::from_secs(last_modified))
                .build();

            if let Some(progress) = &self.progress {
                progress.file_discovered(path);
            }

            files.push(indexable_file);
        }

        Ok(files)
    }
}

/// Metadata for a file discovered during scanning
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
        code(sembly::scanner::forest_root_not_found),
        help("Ensure the path exists and is accessible")
    )]
    ForestRootNotFound { path: String },

    #[snafu(display("Forest root is not a directory: {path}"))]
    #[diagnostic(
        code(sembly::scanner::forest_root_not_directory),
        help("The forest root must be a directory, not a file")
    )]
    ForestRootNotDirectory { path: String },

    #[snafu(display("Error walking directory tree"))]
    #[diagnostic(
        code(sembly::scanner::walk_error),
        help("Check file permissions and filesystem health")
    )]
    Walk { source: ignore::Error },

    #[snafu(display("Permission denied: {path}"))]
    #[diagnostic(
        code(sembly::scanner::permission_denied),
        help("Check file and directory permissions")
    )]
    PermissionDenied { path: String },

    #[snafu(display("I/O error scanning {path}"))]
    #[diagnostic(
        code(sembly::scanner::io_error),
        help("Check that the path is accessible and the filesystem is healthy")
    )]
    IoError {
        path: String,
        source: std::io::Error,
    },

    #[snafu(display("Invalid path structure: {path}"))]
    #[diagnostic(
        code(sembly::scanner::invalid_path_structure),
        help("Ensure the path follows the expected forest directory structure")
    )]
    InvalidPathStructure { path: String },

    #[snafu(display("Failed to create path type"))]
    #[diagnostic(
        code(sembly::scanner::path_creation_failed),
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
            .use_semblyignore(false)
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
    fn test_scanner_rejects_file_as_root() {
        // Given A file path instead of directory
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("not_a_directory.txt");
        fs::write(&file_path, "content").unwrap();

        // When Creating a scanner with file path
        let result = FileScanner::new(&file_path);

        // Then It should fail with ForestRootNotDirectory
        assert!(matches!(
            result,
            Err(ScanError::ForestRootNotDirectory { .. })
        ));
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
    fn test_always_ignores_sembly_directory() {
        // Given A forest with .sembly directory containing indexable files
        let temp_dir = TempDir::new().unwrap();
        let sembly_dir = temp_dir.path().join(".sembly");
        fs::create_dir(&sembly_dir).unwrap();
        fs::write(sembly_dir.join("index.db"), "data").unwrap();
        fs::write(sembly_dir.join("notes.md"), "# Notes").unwrap();

        // When Scanning
        let scanner = FileScanner::new(temp_dir.path()).unwrap();
        let files = scanner.scan().unwrap();

        // Then .sembly directory should never be scanned
        assert!(files.is_empty());
    }

    #[test]
    fn test_scan_config_default_values() {
        // Given Default ScanConfig
        let config = ScanConfig::default();

        // Then It should have expected defaults
        assert!(config.respect_gitignore);
        assert!(config.use_semblyignore);
        assert!(config.targets.is_empty());
    }

    #[test]
    fn test_scanner_with_custom_config() {
        // Given A custom ScanConfig
        let config = ScanConfig::builder()
            .respect_gitignore(false)
            .use_semblyignore(false)
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
    fn test_scan_with_empty_targets_uses_default_behavior() {
        // Given A ScanConfig with empty targets vector
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("test.md"), "# Test").unwrap();

        let config = ScanConfig::builder().targets(vec![]).build();

        // When Scanning
        let scanner = FileScanner::with_config(temp_dir.path(), config).unwrap();
        let files = scanner.scan().unwrap();

        // Then It should scan from forest root
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn test_scan_with_single_target() {
        // Given A forest with multiple directories and config targeting one
        let temp_dir = TempDir::new().unwrap();
        let docs_dir = temp_dir.path().join("docs");
        let bottlerocket_dir = temp_dir.path().join("bottlerocket");
        fs::create_dir(&docs_dir).unwrap();
        fs::create_dir(&bottlerocket_dir).unwrap();
        fs::write(docs_dir.join("guide.md"), "# Guide").unwrap();
        fs::write(bottlerocket_dir.join("README.md"), "# BR").unwrap();

        let config = ScanConfig::builder()
            .targets(vec![PathBuf::from("docs")])
            .build();

        // When Scanning
        let scanner = FileScanner::with_config(temp_dir.path(), config).unwrap();
        let files = scanner.scan().unwrap();

        // Then Only files from target directory should be returned
        assert_eq!(files.len(), 1);
        assert!(files[0].relative_path.to_string().contains("docs"));
    }

    #[test]
    fn test_scan_with_multiple_targets() {
        // Given A forest with several directories and config with multiple targets
        let temp_dir = TempDir::new().unwrap();
        let docs_dir = temp_dir.path().join("docs");
        let skills_dir = temp_dir.path().join("skills");
        let bottlerocket_dir = temp_dir.path().join("bottlerocket");
        fs::create_dir(&docs_dir).unwrap();
        fs::create_dir(&skills_dir).unwrap();
        fs::create_dir(&bottlerocket_dir).unwrap();
        fs::write(docs_dir.join("guide.md"), "# Guide").unwrap();
        fs::write(skills_dir.join("skill.md"), "# Skill").unwrap();
        fs::write(bottlerocket_dir.join("README.md"), "# BR").unwrap();

        let config = ScanConfig::builder()
            .targets(vec![PathBuf::from("docs"), PathBuf::from("skills")])
            .build();

        // When Scanning
        let scanner = FileScanner::with_config(temp_dir.path(), config).unwrap();
        let files = scanner.scan().unwrap();

        // Then Files from all specified targets should be returned
        assert_eq!(files.len(), 2);
        assert!(
            files
                .iter()
                .any(|f| f.relative_path.to_string().contains("docs"))
        );
        assert!(
            files
                .iter()
                .any(|f| f.relative_path.to_string().contains("skills"))
        );
        assert!(
            !files
                .iter()
                .any(|f| f.relative_path.to_string().contains("bottlerocket"))
        );
    }

    #[test]
    fn test_scan_with_nonexistent_target() {
        // Given A ScanConfig with a target that doesn't exist
        let temp_dir = TempDir::new().unwrap();
        let docs_dir = temp_dir.path().join("docs");
        fs::create_dir(&docs_dir).unwrap();
        fs::write(docs_dir.join("guide.md"), "# Guide").unwrap();

        let config = ScanConfig::builder()
            .targets(vec![PathBuf::from("docs"), PathBuf::from("nonexistent")])
            .build();

        // When Scanning
        let scanner = FileScanner::with_config(temp_dir.path(), config).unwrap();
        let files = scanner.scan().unwrap();

        // Then It should skip nonexistent target and return files from valid target
        assert_eq!(files.len(), 1);
        assert!(files[0].relative_path.to_string().contains("docs"));
    }

    #[test]
    fn test_scan_with_overlapping_targets() {
        // Given Targets that overlap
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("repo");
        fs::create_dir_all(repo_dir.join("subdir")).unwrap();
        fs::write(repo_dir.join("top.md"), "# Top").unwrap();
        fs::write(repo_dir.join("subdir/nested.md"), "# Nested").unwrap();

        let config = ScanConfig::builder()
            .targets(vec![PathBuf::from("repo"), PathBuf::from("repo/subdir")])
            .build();

        // When Scanning
        let scanner = FileScanner::with_config(temp_dir.path(), config).unwrap();
        let files = scanner.scan().unwrap();

        // Then It should handle both targets
        assert!(files.len() >= 2);
    }

    #[test]
    fn test_scan_respects_target_specific_gitignore() {
        // Given Multiple targets each with their own .gitignore
        let temp_dir = TempDir::new().unwrap();
        let repo1_dir = temp_dir.path().join("repo1");
        let repo2_dir = temp_dir.path().join("repo2");
        fs::create_dir(&repo1_dir).unwrap();
        fs::create_dir(&repo2_dir).unwrap();

        // Create .git directories
        fs::create_dir(repo1_dir.join(".git")).unwrap();
        fs::create_dir(repo2_dir.join(".git")).unwrap();

        // repo1 ignores "ignored.md"
        fs::write(repo1_dir.join(".gitignore"), "ignored.md\n").unwrap();
        fs::write(repo1_dir.join("included.md"), "# Included").unwrap();
        fs::write(repo1_dir.join("ignored.md"), "# Ignored").unwrap();

        // repo2 ignores "secret.md"
        fs::write(repo2_dir.join(".gitignore"), "secret.md\n").unwrap();
        fs::write(repo2_dir.join("public.md"), "# Public").unwrap();
        fs::write(repo2_dir.join("secret.md"), "# Secret").unwrap();

        let config = ScanConfig::builder()
            .respect_gitignore(true)
            .targets(vec![PathBuf::from("repo1"), PathBuf::from("repo2")])
            .build();

        // When Scanning
        let scanner = FileScanner::with_config(temp_dir.path(), config).unwrap();
        let files = scanner.scan().unwrap();

        // Then Each target should respect its own gitignore
        assert_eq!(files.len(), 2);
        assert!(
            files
                .iter()
                .any(|f| f.relative_path.to_string().contains("included.md"))
        );
        assert!(
            files
                .iter()
                .any(|f| f.relative_path.to_string().contains("public.md"))
        );
        assert!(
            !files
                .iter()
                .any(|f| f.relative_path.to_string().contains("ignored.md"))
        );
        assert!(
            !files
                .iter()
                .any(|f| f.relative_path.to_string().contains("secret.md"))
        );
    }

    #[test]
    fn test_semblyignore_excludes_files() {
        // Given A forest with .semblyignore
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join(".semblyignore"), "excluded/\n").unwrap();
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
    fn test_semblyignore_can_be_disabled() {
        // Given A forest with .semblyignore
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join(".semblyignore"), "excluded/\n").unwrap();
        let repo_dir = temp_dir.path().join("repo");
        fs::create_dir_all(repo_dir.join("excluded")).unwrap();
        fs::write(repo_dir.join("excluded/doc.md"), "# Excluded").unwrap();

        // When Scanning with semblyignore disabled
        let config = ScanConfig::builder().use_semblyignore(false).build();
        let scanner = FileScanner::with_config(temp_dir.path(), config).unwrap();
        let files = scanner.scan().unwrap();

        // Then File should be found
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn test_semblyignore_with_subdirectories() {
        // Given A forest with .semblyignore excluding a subdirectory
        let temp_dir = TempDir::new().unwrap();

        // Create .git directory to prevent global gitignore interference
        fs::create_dir(temp_dir.path().join(".git")).unwrap();

        fs::write(temp_dir.path().join(".semblyignore"), "*/vendor/\n").unwrap();
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
    fn test_semblyignore_wildcard_patterns() {
        // Given A forest with .semblyignore using wildcards
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join(".semblyignore"), "*.tmp.md\n").unwrap();
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

    #[test]
    #[cfg(unix)]
    fn test_scanner_does_not_follow_symlinks() {
        // Given A forest with a symlink pointing outside
        let temp_dir = TempDir::new().unwrap();
        let outside_dir = TempDir::new().unwrap();

        // Create file outside forest
        fs::write(outside_dir.path().join("external.md"), "# External").unwrap();

        // Create repo with symlink to external file
        let repo_dir = temp_dir.path().join("repo");
        fs::create_dir(&repo_dir).unwrap();
        fs::write(repo_dir.join("internal.md"), "# Internal").unwrap();

        #[cfg(unix)]
        std::os::unix::fs::symlink(
            outside_dir.path().join("external.md"),
            repo_dir.join("link.md"),
        )
        .unwrap();

        // When Scanning
        let scanner = FileScanner::new(temp_dir.path()).unwrap();
        let files = scanner.scan().unwrap();

        // Then Only internal file should be found (symlink not followed)
        assert_eq!(files.len(), 1);
        assert!(files[0].relative_path.to_string().contains("internal.md"));
        assert!(
            !files
                .iter()
                .any(|f| f.relative_path.to_string().contains("link.md"))
        );
        assert!(
            !files
                .iter()
                .any(|f| f.relative_path.to_string().contains("external.md"))
        );
    }
}
