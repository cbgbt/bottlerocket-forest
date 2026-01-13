#![expect(clippy::unwrap_used, reason = "panics are appropriate in tests")]
//! Integration tests for forester seed and grove commands.
//!
//! These tests use local git repos to avoid network dependencies.

mod common;

use common::{create_bare_repo, forester_grove, forester_seed, temp_forest};
use std::fs;

fn setup_forest_with_local_repos() -> tempfile::TempDir {
    let temp = temp_forest();

    // Create local bare repos to use as "remotes"
    let repos_dir = temp.path().join("repos");
    fs::create_dir_all(&repos_dir).unwrap();

    let repo_a = create_bare_repo(&repos_dir, "repo-a");
    let repo_b = create_bare_repo(&repos_dir, "repo-b");

    // Create forester.toml pointing to local repos
    let forester_toml = format!(
        r#"[forest]
name = "test-forest"

[[forest.member]]
name = "repo-a"
remote = "{}"
path = "repo-a"
default_branch = "main"

[[forest.member]]
name = "repo-b"
remote = "{}"
path = "nested/repo-b"
default_branch = "main"
"#,
        repo_a.display(),
        repo_b.display()
    );
    fs::write(temp.path().join("forester.toml"), forester_toml).unwrap();

    // Create crumbly.toml
    fs::write(temp.path().join("crumbly.toml"), "targets = [\".\"]\n").unwrap();

    temp
}

#[test]
fn seed_clones_bare_repos() {
    // Given: A forest with local repo remotes
    let temp = setup_forest_with_local_repos();

    // When: Running forester seed
    let (code, stdout, stderr) = forester_seed(temp.path(), true);

    // Then: Command succeeds
    assert_eq!(code, 0, "Seed failed: {} {}", stdout, stderr);

    // And: Bare repos are created
    assert!(temp.path().join(".forest/bare/repo-a.git").exists());
    assert!(temp.path().join(".forest/bare/repo-b.git").exists());
}

#[test]
fn seed_does_not_create_develop_grove() {
    // Given: A forest with local repo remotes
    let temp = setup_forest_with_local_repos();

    // When: Running forester seed
    let (code, _, _) = forester_seed(temp.path(), false);
    assert_eq!(code, 0);

    // Then: No develop grove exists (hidden grove used for indexing is cleaned up)
    assert!(!temp.path().join("groves/develop").exists());

    // And: Hidden index grove is also cleaned up
    assert!(!temp.path().join(".forest/.index-grove").exists());
}

#[test]
fn seed_is_idempotent() {
    // Given: A forest that has already been seeded
    let temp = setup_forest_with_local_repos();
    let (code, _, _) = forester_seed(temp.path(), false);
    assert_eq!(code, 0);

    // When: Running forester seed again
    let (code, stdout, stderr) = forester_seed(temp.path(), true);

    // Then: Command succeeds
    assert_eq!(code, 0, "Second seed failed: {} {}", stdout, stderr);

    // And: Shows repos already exist (output goes to stderr)
    assert!(
        stdout.contains("already exists")
            || stdout.contains("✓")
            || stderr.contains("already exists")
            || stderr.contains("✓")
    );
}

#[test]
fn grove_create_makes_new_grove() {
    // Given: A seeded forest
    let temp = setup_forest_with_local_repos();
    let (code, _, _) = forester_seed(temp.path(), false);
    assert_eq!(code, 0);

    // When: Creating a new grove
    let (code, stdout, stderr) = forester_grove(temp.path(), "create", &["feature-x"]);

    // Then: Command succeeds
    assert_eq!(code, 0, "Grove create failed: {} {}", stdout, stderr);

    // And: New grove directory exists
    assert!(temp.path().join("groves/feature-x").exists());
    assert!(temp.path().join("groves/feature-x/repo-a").exists());
    assert!(temp.path().join("groves/feature-x/nested/repo-b").exists());
}

#[test]
fn grove_list_shows_groves() {
    // Given: A seeded forest with a created grove
    let temp = setup_forest_with_local_repos();
    forester_seed(temp.path(), false);
    forester_grove(temp.path(), "create", &["feature-x"]);

    // When: Listing groves
    let (code, stdout, _) = forester_grove(temp.path(), "list", &[]);

    // Then: Created grove is listed
    assert_eq!(code, 0);
    assert!(stdout.contains("feature-x"));
}

#[test]
fn grove_remove_deletes_grove() {
    // Given: A seeded forest with two groves
    let temp = setup_forest_with_local_repos();
    forester_seed(temp.path(), false);
    forester_grove(temp.path(), "create", &["feature-x"]);
    forester_grove(temp.path(), "create", &["feature-y"]);
    assert!(temp.path().join("groves/feature-x").exists());
    assert!(temp.path().join("groves/feature-y").exists());

    // When: Removing one grove
    let (code, stdout, stderr) = forester_grove(temp.path(), "remove", &["feature-x"]);

    // Then: Command succeeds
    assert_eq!(code, 0, "Grove remove failed: {} {}", stdout, stderr);

    // And: Removed grove is gone
    assert!(!temp.path().join("groves/feature-x").exists());

    // And: Other grove still exists
    assert!(temp.path().join("groves/feature-y").exists());
}

#[test]
fn no_repos_at_forest_root() {
    // Given: A seeded forest
    let temp = setup_forest_with_local_repos();
    forester_seed(temp.path(), false);

    // Then: No repo directories at forest root (only in groves/)
    assert!(!temp.path().join("repo-a").exists());
    assert!(!temp.path().join("nested").exists());

    // And: Only config files and directories at root
    let entries: Vec<_> = fs::read_dir(temp.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();

    // Should only have: forester.toml, crumbly.toml, .forest, .crumbly, groves, repos (our test remotes)
    for entry in &entries {
        assert!(
            [
                "forester.toml",
                "crumbly.toml",
                ".forest",
                ".crumbly",
                "groves",
                "repos"
            ]
            .contains(&entry.as_str()),
            "Unexpected entry at forest root: {}",
            entry
        );
    }
}
