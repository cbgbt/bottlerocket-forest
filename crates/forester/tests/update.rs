//! Integration tests for forester update command.

mod common;

use common::{create_bare_repo, forester_seed, forester_update, temp_forest};
use std::fs;
use std::process::Command;

fn setup_forest_with_local_repos() -> (tempfile::TempDir, std::path::PathBuf) {
    let temp = temp_forest();

    let repos_dir = temp.path().join("repos");
    fs::create_dir_all(&repos_dir).unwrap();

    let repo_a = create_bare_repo(&repos_dir, "repo-a");

    let forester_toml = format!(
        r#"[forest]
name = "test-forest"

[[forest.member]]
name = "repo-a"
remote = "{}"
path = "repo-a"
default_branch = "main"
"#,
        repo_a.display()
    );
    fs::write(temp.path().join("forester.toml"), forester_toml).unwrap();
    fs::write(temp.path().join("crumbly.toml"), "targets = [\".\"]\n").unwrap();

    (temp, repo_a)
}

fn add_commit_to_bare_repo(bare_repo: &std::path::Path, message: &str) {
    let temp = tempfile::TempDir::new().unwrap();

    Command::new("git")
        .args(["clone", bare_repo.to_str().unwrap(), "."])
        .current_dir(temp.path())
        .output()
        .unwrap();

    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(temp.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(temp.path())
        .output()
        .unwrap();

    fs::write(temp.path().join("new-file.txt"), message).unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(temp.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(temp.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(["push", "origin", "main"])
        .current_dir(temp.path())
        .output()
        .unwrap();
}

#[test]
fn update_fetches_new_commits() {
    let (temp, remote_repo) = setup_forest_with_local_repos();

    let (code, _, _) = forester_seed(temp.path(), false);
    assert_eq!(code, 0);

    add_commit_to_bare_repo(&remote_repo, "new commit");

    let (code, stdout, stderr) = forester_update(temp.path(), true);
    assert_eq!(code, 0, "Update failed: {} {}", stdout, stderr);

    let output = Command::new("git")
        .args(["log", "--oneline", "refs/remotes/origin/main"])
        .current_dir(temp.path().join(".forest/bare/repo-a.git"))
        .output()
        .unwrap();
    let log = String::from_utf8_lossy(&output.stdout);
    assert!(log.contains("new commit"));
}

#[test]
fn update_succeeds_with_no_changes() {
    let (temp, _) = setup_forest_with_local_repos();

    let (code, _, _) = forester_seed(temp.path(), false);
    assert_eq!(code, 0);

    let (code, stdout, stderr) = forester_update(temp.path(), true);
    assert_eq!(code, 0, "Update failed: {} {}", stdout, stderr);
    assert!(stdout.contains("updated") || stdout.contains("✓"));
}

#[test]
fn update_fails_if_not_seeded() {
    let temp = temp_forest();

    let forester_toml = r#"[forest]
name = "test-forest"

[[forest.member]]
name = "repo-a"
remote = "/nonexistent"
path = "repo-a"
"#;
    fs::write(temp.path().join("forester.toml"), forester_toml).unwrap();

    let (code, _, stderr) = forester_update(temp.path(), false);
    assert_ne!(code, 0);
    assert!(stderr.contains("not seeded") || stderr.contains("seed"));
}
