//! Integration tests for schema migration behavior.

mod common;

use rusqlite::Connection;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn crumbly_cmd() -> Command {
    Command::new(common::crumbly_bin())
}

/// Schema v4 used L2 (Euclidean) distance for vector search, which produced
/// incorrectly low similarity scores. v5 switches to cosine distance via DDL.
/// Since sqlite-vec virtual tables cannot be altered in place, users must
/// rebuild their index to get correct scoring behavior.
#[test]
fn test_v4_database_shows_rebuild_suggestion() {
    // Given A v4 database (pre-cosine-distance fix)
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    fs::write(root.join("test.md"), "# Test\n\nContent").unwrap();

    let crumbly_dir = root.join(".crumbly");
    fs::create_dir_all(&crumbly_dir).unwrap();

    let db_path = crumbly_dir.join("knowledge.db");
    let conn = Connection::open(&db_path).unwrap();
    conn.execute(
        "CREATE TABLE index_metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO index_metadata (key, value) VALUES ('schema_version', '4')",
        [],
    )
    .unwrap();
    drop(conn);

    // When Attempting any index operation
    let output = crumbly_cmd()
        .args(["search", "test"])
        .current_dir(root)
        .output()
        .unwrap();

    // Then It fails with a helpful rebuild suggestion
    assert!(!output.status.success(), "Expected failure for v4 database");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.to_lowercase().contains("rebuild"),
        "Expected 'rebuild' suggestion in error, got: {}",
        stderr
    );
}
