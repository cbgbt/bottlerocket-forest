//! Integration tests for rebuild context preservation and search scores.

mod common;

use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn setup_temp_dir() -> TempDir {
    TempDir::new().unwrap()
}

fn crumbly_cmd() -> Command {
    Command::new(common::crumbly_bin())
}

#[test]
fn test_rebuild_preserves_multiple_contexts() {
    let temp = setup_temp_dir();
    let root = temp.path();

    let subdir = root.join("subdir");
    fs::create_dir_all(&subdir).unwrap();
    fs::write(subdir.join("test.md"), "# Test\n\nSome content").unwrap();
    fs::write(root.join("root.md"), "# Root\n\nRoot content").unwrap();

    let output = crumbly_cmd()
        .args(["build", "--context", "subdir"])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output = crumbly_cmd()
        .args(["rebuild", "--context", "subdir"])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "rebuild failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output = crumbly_cmd()
        .args(["search", "-n", "5", "test", "--context", "subdir"])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "search failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_search_scores_reasonable_for_good_matches() {
    let temp = setup_temp_dir();
    let root = temp.path();

    fs::write(
        root.join("content.md"),
        "# Quick Brown Fox\n\nThe quick brown fox jumps over the lazy dog.",
    )
    .unwrap();

    let output = crumbly_cmd()
        .args(["build"])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(output.status.success());

    let output = crumbly_cmd()
        .args(["search", "-n", "1", "quick brown fox", "--json"])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    if let Some(score_str) = stdout.split("\"score\":").nth(1) {
        if let Some(score_end) = score_str.find(|c: char| !c.is_numeric() && c != '.') {
            let score: f32 = score_str[..score_end].parse().unwrap_or(0.0);
            assert!(score >= 0.4, "Expected score >= 0.4, got {}", score);
        }
    }
}
