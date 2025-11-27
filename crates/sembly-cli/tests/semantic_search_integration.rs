//! Integration tests for semantic search using fixture directories.
//!
//! Each fixture is a self-contained workspace with `.sembly.toml` and source files.
//! Tests copy fixtures to temp directories, build the index, and verify search behavior.
//!
//! These tests are marked with `#[ignore]` because they:
//! - Download embedding models from the internet (slow, network-dependent)
//! - Require significant disk space for model caching
//! - Take several seconds to run per test
//!
//! Run with: `cargo test --test semantic_search_integration -- --ignored`

use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

/// Get path to sembly binary
fn sembly_bin() -> &'static Path {
    assert_cmd::cargo::cargo_bin!("sembly")
}

/// Copies a fixture directory to a temp location for testing
fn setup_fixture(fixture_name: &str) -> TempDir {
    let temp = TempDir::new().unwrap();
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(fixture_name);

    copy_dir_recursive(&fixture_path, temp.path()).unwrap();
    temp
}

/// Recursively copy a directory
fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// Run sembly build in the given directory
fn sembly_build(workspace: &Path) -> (i32, String, String) {
    let output = Command::new(sembly_bin())
        .current_dir(workspace)
        .args(["build"])
        .output()
        .expect("Failed to execute sembly");

    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    (exit_code, stdout, stderr)
}

/// Run sembly build with --context flag
fn sembly_build_context(workspace: &Path, context: &str) -> (i32, String, String) {
    let output = Command::new(sembly_bin())
        .current_dir(workspace)
        .args(["build", "--context", context])
        .output()
        .expect("Failed to execute sembly");

    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    (exit_code, stdout, stderr)
}

/// Run sembly search with --show-chunks flag
fn sembly_search_with_chunks(workspace: &Path, query: &str) -> (i32, String, String) {
    let output = Command::new(sembly_bin())
        .current_dir(workspace)
        .args(["search", "--show-chunks", query])
        .output()
        .expect("Failed to execute sembly");

    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    (exit_code, stdout, stderr)
}

/// Run sembly update with --context flag (for adding additional contexts)
fn sembly_update_context(workspace: &Path, context: &str) -> (i32, String, String) {
    let output = Command::new(sembly_bin())
        .current_dir(workspace)
        .args(["update", "--context", context])
        .output()
        .expect("Failed to execute sembly");

    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    (exit_code, stdout, stderr)
}

/// Run sembly search with --context flag
fn sembly_search_context(workspace: &Path, query: &str, context: &str) -> (i32, String, String) {
    let output = Command::new(sembly_bin())
        .current_dir(workspace)
        .args(["search", "--show-chunks", "--context", context, query])
        .output()
        .expect("Failed to execute sembly");

    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    (exit_code, stdout, stderr)
}

// =============================================================================
// Basic Rust Indexing Tests
// =============================================================================

#[test]
#[ignore]
fn rust_basic_indexes_and_searches() {
    // Given: A workspace with basic Rust files
    let workspace = setup_fixture("rust_basic");

    // When: Building the index
    let (code, stdout, stderr) = sembly_build(workspace.path());

    // Then: Build succeeds
    assert_eq!(code, 0, "Build failed: {}\n{}", stdout, stderr);
    assert!(stdout.contains("complete") || stdout.contains("Complete"));

    // When: Searching for startup-related content
    let (code, stdout, _) =
        sembly_search_with_chunks(workspace.path(), "startup initialization sequence");

    // Then: Search succeeds and finds relevant content
    assert_eq!(code, 0, "Search failed");
    assert!(
        stdout.contains("Found") && stdout.contains("unique files"),
        "Should find results: {}",
        stdout
    );
    assert!(
        stdout.contains("startup") || stdout.contains("Startup") || stdout.contains("initialize"),
        "Should find startup content: {}",
        stdout
    );
}

#[test]
#[ignore]
fn rust_basic_search_returns_relevant_chunks() {
    // Given: An indexed workspace
    let workspace = setup_fixture("rust_basic");
    let (code, _, stderr) = sembly_build(workspace.path());
    assert_eq!(code, 0, "Build failed: {}", stderr);

    // When: Searching with --show-chunks
    let (code, stdout, _) =
        sembly_search_with_chunks(workspace.path(), "network connection pooling");

    // Then: Results include the networking function content
    assert_eq!(code, 0, "Search failed");
    assert!(
        stdout.contains("network") || stdout.contains("connection"),
        "Should find networking content: {}",
        stdout
    );
}

// =============================================================================
// Rust Visibility Filtering Tests
// =============================================================================

#[test]
#[ignore]
fn rust_visibility_indexes_only_public_items() {
    // Given: A workspace with visibility = "public"
    let workspace = setup_fixture("rust_visibility");
    let (code, _, stderr) = sembly_build(workspace.path());
    assert_eq!(code, 0, "Build failed: {}", stderr);

    // When: Searching for public function
    let (code, stdout, _) = sembly_search_with_chunks(workspace.path(), "initialization app");

    // Then: Public function is found
    assert_eq!(code, 0, "Search failed");
    assert!(
        stdout.contains("init") || stdout.contains("app"),
        "Should find public init_app: {}",
        stdout
    );
}

#[test]
#[ignore]
fn rust_visibility_excludes_private_items() {
    // Given: A workspace with visibility = "public"
    let workspace = setup_fixture("rust_visibility");
    let (code, _, stderr) = sembly_build(workspace.path());
    assert_eq!(code, 0, "Build failed: {}", stderr);

    // When: Searching for private function content
    let (code, stdout, _) =
        sembly_search_with_chunks(workspace.path(), "memory pools caching layers allocates");

    // Then: Private function should not appear in results
    assert_eq!(code, 0, "Search failed");
    assert!(
        !stdout.contains("setup_memory"),
        "Should NOT find private setup_memory: {}",
        stdout
    );
}

// =============================================================================
// Rust Item Type Filtering Tests
// =============================================================================

#[test]
#[ignore]
fn rust_item_types_indexes_functions_and_structs() {
    // Given: A workspace with item_types = ["function", "struct"]
    let workspace = setup_fixture("rust_item_types");
    let (code, _, stderr) = sembly_build(workspace.path());
    assert_eq!(code, 0, "Build failed: {}", stderr);

    // When: Searching for function content
    let (code, stdout, _) = sembly_search_with_chunks(workspace.path(), "process request handler");

    // Then: Function is found
    assert_eq!(code, 0, "Search failed");
    assert!(
        stdout.contains("process") || stdout.contains("request"),
        "Should find function: {}",
        stdout
    );

    // When: Searching for struct content
    let (code, stdout, _) = sembly_search_with_chunks(workspace.path(), "request path method");

    // Then: Struct is found
    assert_eq!(code, 0, "Search failed");
    assert!(
        stdout.contains("Request") || stdout.contains("request"),
        "Should find struct: {}",
        stdout
    );
}

#[test]
#[ignore]
fn rust_item_types_excludes_enums_and_traits() {
    // Given: A workspace with item_types = ["function", "struct"]
    let workspace = setup_fixture("rust_item_types");
    let (code, _, stderr) = sembly_build(workspace.path());
    assert_eq!(code, 0, "Build failed: {}", stderr);

    // When: Searching for enum content
    let (code, stdout, _) =
        sembly_search_with_chunks(workspace.path(), "response status ok bad request error");

    // Then: Enum should not be in results
    assert_eq!(code, 0, "Search failed");
    assert!(
        !stdout.contains("ResponseStatus"),
        "Should NOT find enum ResponseStatus: {}",
        stdout
    );

    // When: Searching for trait content
    let (code, stdout, _) =
        sembly_search_with_chunks(workspace.path(), "handler trait implement endpoint");

    // Then: Trait should not be in results
    assert_eq!(code, 0, "Search failed");
    assert!(
        !stdout.contains("RequestHandler"),
        "Should NOT find trait RequestHandler: {}",
        stdout
    );
}

// =============================================================================
// Markdown Chunking Tests
// =============================================================================

#[test]
#[ignore]
fn markdown_docs_indexes_and_searches() {
    // Given: A workspace with markdown documentation
    let workspace = setup_fixture("markdown_docs");
    let (code, _, stderr) = sembly_build(workspace.path());
    assert_eq!(code, 0, "Build failed: {}", stderr);

    // When: Searching for startup process content
    let (code, stdout, _) =
        sembly_search_with_chunks(workspace.path(), "startup process stages configuration");

    // Then: Startup process content is found
    assert_eq!(code, 0, "Search failed");
    assert!(
        stdout.contains("Startup")
            || stdout.contains("startup")
            || stdout.contains("initialization"),
        "Should find startup process content: {}",
        stdout
    );
}

#[test]
#[ignore]
fn markdown_docs_finds_configuration_content() {
    // Given: An indexed markdown workspace
    let workspace = setup_fixture("markdown_docs");
    let (code, _, stderr) = sembly_build(workspace.path());
    assert_eq!(code, 0, "Build failed: {}", stderr);

    // When: Searching for database configuration
    let (code, stdout, _) =
        sembly_search_with_chunks(workspace.path(), "database connection settings host port");

    // Then: Database configuration content is found
    assert_eq!(code, 0, "Search failed");
    assert!(
        stdout.contains("database") || stdout.contains("Database") || stdout.contains("connection"),
        "Should find database content: {}",
        stdout
    );
}

// =============================================================================
// Boost Rules Tests
// =============================================================================

#[test]
#[ignore]
fn boost_rules_build_succeeds() {
    // Given: A workspace with boost rules
    let workspace = setup_fixture("boost_rules");

    // When: Building the index
    let (code, stdout, stderr) = sembly_build(workspace.path());

    // Then: Build succeeds
    assert_eq!(code, 0, "Build failed: {}\n{}", stdout, stderr);

    // When: Searching for content that exists in both src/ and docs/
    let (code, stdout, _) =
        sembly_search_with_chunks(workspace.path(), "startup process handler system");

    // Then: Search succeeds and finds startup content
    assert_eq!(code, 0, "Search failed");
    assert!(
        stdout.contains("startup") || stdout.contains("Startup"),
        "Should find startup content: {}",
        stdout
    );
}

// =============================================================================
// Deduplication Tests
// =============================================================================

#[test]
#[ignore]
fn deduplication_handles_identical_headers() {
    // Given: A workspace with files sharing identical license headers
    let workspace = setup_fixture("deduplication");

    // When: Building the index
    let (code, stdout, stderr) = sembly_build(workspace.path());

    // Then: Build succeeds without duplicate key errors
    assert_eq!(
        code, 0,
        "Build should succeed with deduplication: {}\n{}",
        stdout, stderr
    );
    assert!(
        !stderr.contains("UNIQUE constraint"),
        "Should not have duplicate key errors"
    );
}

#[test]
#[ignore]
fn deduplication_rebuild_succeeds() {
    // Given: An existing index
    let workspace = setup_fixture("deduplication");
    let (code, _, stderr) = sembly_build(workspace.path());
    assert_eq!(code, 0, "Initial build failed: {}", stderr);

    // When: Rebuilding the index
    let output = Command::new(sembly_bin())
        .current_dir(workspace.path())
        .args(["rebuild"])
        .output()
        .expect("Failed to execute sembly");

    // Then: Rebuild succeeds
    assert_eq!(
        output.status.code().unwrap_or(-1),
        0,
        "Rebuild should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// =============================================================================
// Context Tests
// =============================================================================

#[test]
#[ignore]
fn context_multiple_contexts_can_be_indexed() {
    // Given: A workspace with multiple context directories
    let workspace = setup_fixture("context_boost");

    // When: Building the main context first (creates the index)
    let (code, stdout, stderr) = sembly_build_context(workspace.path(), "main");
    assert_eq!(code, 0, "Main context build failed: {}\n{}", stdout, stderr);

    // When: Adding the worktree context using update
    let (code, stdout, stderr) = sembly_update_context(workspace.path(), "worktree");
    assert_eq!(
        code, 0,
        "Worktree context update failed: {}\n{}",
        stdout, stderr
    );

    // Then: Both contexts should be searchable
    let (code, stdout, _) =
        sembly_search_with_chunks(workspace.path(), "database connection handler");
    assert_eq!(code, 0, "Search failed");
    assert!(
        stdout.contains("database") || stdout.contains("Database"),
        "Should find database content: {}",
        stdout
    );
}

#[test]
#[ignore]
fn context_list_shows_registered_contexts() {
    // Given: A workspace with multiple contexts
    let workspace = setup_fixture("context_boost");
    let (code, _, _) = sembly_build_context(workspace.path(), "main");
    assert_eq!(code, 0);
    let (code, _, _) = sembly_update_context(workspace.path(), "worktree");
    assert_eq!(code, 0);

    // When: Listing contexts
    let output = Command::new(sembly_bin())
        .current_dir(workspace.path())
        .args(["context", "list"])
        .output()
        .expect("Failed to execute sembly");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    // Then: Both contexts should be listed
    assert_eq!(output.status.code().unwrap_or(-1), 0, "Context list failed");
    assert!(
        stdout.contains("main"),
        "Should list main context: {}",
        stdout
    );
    assert!(
        stdout.contains("worktree"),
        "Should list worktree context: {}",
        stdout
    );
}

#[test]
#[ignore]
fn context_isolation_prevents_cross_context_results() {
    // BUG: Context filtering in search is not working correctly.
    // Searching with --context should only return results from that context,
    // but currently returns results from all contexts.

    // Given: A workspace with two contexts containing different content
    let workspace = setup_fixture("context_boost");

    // Build main context first, then add worktree context
    let (code, _, stderr) = sembly_build_context(workspace.path(), "main");
    assert_eq!(code, 0, "Main context build failed: {}", stderr);
    let (code, _, stderr) = sembly_update_context(workspace.path(), "worktree");
    assert_eq!(code, 0, "Worktree context update failed: {}", stderr);

    // When: Searching in main context for content that only exists in worktree
    // ("async connection pool" is only in worktree/docs/database.md)
    let (code, stdout, _) = sembly_search_context(
        workspace.path(),
        "async connection pool new strategy",
        "main",
    );

    // Then: Should NOT find worktree-specific content when searching main context
    assert_eq!(code, 0, "Search failed");
    assert!(
        !stdout.contains("async") && !stdout.contains("new strategy"),
        "Main context should not leak worktree content: {}",
        stdout
    );
}
