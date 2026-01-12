#![allow(dead_code)]
//! Common test utilities for forester integration tests.

use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

/// Get path to forester binary
pub fn forester_bin() -> &'static Path {
    assert_cmd::cargo::cargo_bin!("forester")
}

/// Run forester command in the given directory
pub fn forester_cmd(cwd: &Path, args: &[&str]) -> (i32, String, String) {
    let output = Command::new(forester_bin())
        .current_dir(cwd)
        .args(args)
        .output()
        .expect("Failed to execute forester");

    output_to_tuple(output)
}

/// Run forester init in the given directory
pub fn forester_init(cwd: &Path, name: Option<&str>) -> (i32, String, String) {
    let mut args = vec!["init"];
    if let Some(n) = name {
        args.push("--name");
        args.push(n);
    }
    forester_cmd(cwd, &args)
}

/// Run forester seed in the given directory
pub fn forester_seed(cwd: &Path, verbose: bool) -> (i32, String, String) {
    let mut args = vec!["seed"];
    if verbose {
        args.push("--verbose");
    }
    forester_cmd(cwd, &args)
}

/// Run forester grove command
pub fn forester_grove(cwd: &Path, subcmd: &str, args: &[&str]) -> (i32, String, String) {
    let mut full_args = vec!["grove", subcmd];
    full_args.extend(args);
    forester_cmd(cwd, &full_args)
}

/// Run forester update in the given directory
pub fn forester_update(cwd: &Path, verbose: bool) -> (i32, String, String) {
    let mut args = vec!["update"];
    if verbose {
        args.push("--verbose");
    }
    forester_cmd(cwd, &args)
}

/// Create a temp directory for testing
pub fn temp_forest() -> TempDir {
    TempDir::new().expect("Failed to create temp directory")
}

/// Initialize a git repo in the given directory
pub fn git_init(path: &Path) {
    Command::new("git")
        .current_dir(path)
        .args(["init"])
        .output()
        .expect("Failed to git init");
}

/// Create a bare git repo with a commit
pub fn create_bare_repo(path: &Path, name: &str) -> std::path::PathBuf {
    let repo_path = path.join(format!("{}.git", name));

    // Create a temp repo, add a commit, then clone as bare
    let temp = TempDir::new().unwrap();
    git_init(temp.path());

    // Configure git user for commit
    Command::new("git")
        .current_dir(temp.path())
        .args(["config", "user.email", "test@test.com"])
        .output()
        .unwrap();
    Command::new("git")
        .current_dir(temp.path())
        .args(["config", "user.name", "Test"])
        .output()
        .unwrap();

    // Create initial commit
    std::fs::write(temp.path().join("README.md"), "# Test").unwrap();
    Command::new("git")
        .current_dir(temp.path())
        .args(["add", "."])
        .output()
        .unwrap();
    Command::new("git")
        .current_dir(temp.path())
        .args(["commit", "-m", "Initial commit"])
        .output()
        .unwrap();
    Command::new("git")
        .current_dir(temp.path())
        .args(["branch", "-M", "main"])
        .output()
        .unwrap();

    // Clone as bare
    Command::new("git")
        .args(["clone", "--bare"])
        .arg(temp.path())
        .arg(&repo_path)
        .output()
        .expect("Failed to create bare repo");

    repo_path
}

fn output_to_tuple(output: Output) -> (i32, String, String) {
    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (exit_code, stdout, stderr)
}
