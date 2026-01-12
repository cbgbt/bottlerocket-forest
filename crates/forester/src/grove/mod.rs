//! Forest grove management.

mod context;
pub use context::{GroveContext, GroveContextError};

use crate::error::Error;
use crate::forest::{ForestConfig, Member};
use owo_colors::OwoColorize;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tracing::instrument;

#[derive(Debug)]
pub struct ForestManager {
    root: PathBuf,
    config: ForestConfig,
}

impl ForestManager {
    pub fn new(root: PathBuf, config: ForestConfig) -> Self {
        Self { root, config }
    }

    fn bare_dir(&self) -> PathBuf {
        self.root.join(".forest").join("bare")
    }

    fn groves_dir(&self) -> PathBuf {
        self.root.join("groves")
    }

    #[instrument(skip(self), err)]
    pub fn seed(&self, verbose: bool) -> Result<(), Error> {
        let bare_dir = self.bare_dir();
        std::fs::create_dir_all(&bare_dir).map_err(|e| Error::CreateDir {
            path: bare_dir.clone(),
            source: e,
        })?;

        for member in &self.config.forest.member {
            self.clone_bare(member, verbose)?;
        }

        self.ensure_crumbly_config(verbose)?;

        let index_grove = self.root.join(".forest").join(".index-grove");
        self.create_grove_at(&index_grove, verbose)?;
        self.remove_grove_at(&index_grove);

        Ok(())
    }

    /// Update all bare repos by fetching from remotes.
    #[instrument(skip(self), err)]
    pub fn update(&self, verbose: bool) -> Result<(), Error> {
        let bare_dir = self.bare_dir();
        if !bare_dir.exists() {
            return Err(Error::Git {
                message: "Forest not seeded. Run 'forester seed' first.".to_string(),
            });
        }

        for member in &self.config.forest.member {
            let bare_path = bare_dir.join(format!("{}.git", member.name));
            if !bare_path.exists() {
                if verbose {
                    println!(
                        "{} {} bare repo missing, skipping",
                        "!".yellow(),
                        member.name
                    );
                }
                continue;
            }

            if verbose {
                println!("Fetching {}...", member.name);
            }

            let status = Command::new("git")
                .args([
                    "fetch",
                    &member.remote,
                    "+refs/heads/*:refs/remotes/origin/*",
                    "--prune",
                ])
                .current_dir(&bare_path)
                .status()
                .map_err(|_| Error::Git {
                    message: format!("Failed to fetch {}", member.name),
                })?;

            if !status.success() {
                return Err(Error::Git {
                    message: format!("git fetch failed for {}", member.name),
                });
            }

            if verbose {
                println!("{} {} updated", "✓".green(), member.name);
            }
        }

        Ok(())
    }

    #[instrument(skip(self), err)]
    fn clone_bare(&self, member: &Member, verbose: bool) -> Result<(), Error> {
        let bare_path = self.bare_dir().join(format!("{}.git", member.name));

        if !bare_path.exists() {
            if verbose {
                println!("Cloning {} (bare)...", member.name);
            }
            let mut cmd = Command::new("git");
            cmd.args(["clone", "--bare", &member.remote])
                .arg(&bare_path);
            if !verbose {
                cmd.stdout(Stdio::null()).stderr(Stdio::null());
            }
            let status = cmd.status().map_err(|_| Error::Git {
                message: format!("Failed to clone {}", member.remote),
            })?;
            if !status.success() {
                return Err(Error::Git {
                    message: format!("git clone failed for {}", member.name),
                });
            }
        } else if verbose {
            println!("{} {} bare repo exists", "\u{2713}".green(), member.name);
        }

        Ok(())
    }

    fn ensure_crumbly_config(&self, verbose: bool) -> Result<(), Error> {
        let crumbly_path = self.root.join("crumbly.toml");
        let member_paths: Vec<String> = self
            .config
            .forest
            .member
            .iter()
            .map(|m| m.path.display().to_string())
            .collect();

        if !crumbly_path.exists() {
            let config = Self::generate_crumbly_config(&member_paths);
            std::fs::write(&crumbly_path, config).map_err(|e| Error::CreateDir {
                path: crumbly_path.clone(),
                source: e,
            })?;
            if verbose {
                println!(
                    "{} Generated crumbly.toml with {} targets",
                    "\u{2713}".green(),
                    member_paths.len()
                );
            }
        } else {
            let content = std::fs::read_to_string(&crumbly_path).unwrap_or_default();
            let targets: Vec<&str> = content
                .lines()
                .map(str::trim)
                .filter(|t| t.starts_with('"'))
                .map(|t| t.trim_matches(|c| c == '"' || c == ',' || c == ' '))
                .collect();
            let missing: Vec<_> = member_paths
                .iter()
                .filter(|p| {
                    !targets
                        .iter()
                        .any(|t| p.as_str() == *t || p.starts_with(&format!("{}/", t)))
                })
                .collect();
            if !missing.is_empty() {
                println!(
                    "{} Some members not in crumbly.toml targets: {}",
                    "!".yellow(),
                    missing
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
        }
        Ok(())
    }

    fn generate_crumbly_config(member_paths: &[String]) -> String {
        let targets: Vec<String> = member_paths
            .iter()
            .map(|p| format!("    \"{}\"", p))
            .collect();
        format!(
            r#"# Sembly configuration for this forest
# Generated by forester seed

# Directories to index (your forest members)
targets = [
{},
]

# Uncomment to customize file types (default: markdown, rust)
# enabled-file-types = ["markdown", "rust"]

# Uncomment to configure Rust indexing
# [file-types.rust]
# visibility = ["public"]  # Options: public, crate, private
# items = ["all"]  # Options: all, modules, functions, structs, enums, traits, impls, type-aliases, constants
# min-doc-lines = 0

# Uncomment to add boost rules for search ranking
# [[boost-rules]]
# description = "Prioritize documentation"
# pattern = "**/docs/**"
# multiplier = 1.2
"#,
            targets.join(",\n")
        )
    }

    fn create_grove_at(&self, path: &PathBuf, verbose: bool) -> Result<(), Error> {
        std::fs::create_dir_all(path).map_err(|e| Error::CreateDir {
            path: path.clone(),
            source: e,
        })?;

        for member in &self.config.forest.member {
            let bare_path = self.bare_dir().join(format!("{}.git", member.name));
            let member_wt_path = path.join(&member.path);
            if member_wt_path.exists() {
                continue;
            }
            if let Some(parent) = member_wt_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| Error::CreateDir {
                    path: parent.to_path_buf(),
                    source: e,
                })?;
            }
            let mut cmd = Command::new("git");
            cmd.args(["worktree", "add"])
                .arg(&member_wt_path)
                .arg(member.branch())
                .current_dir(&bare_path);
            if !verbose {
                cmd.stdout(Stdio::null()).stderr(Stdio::null());
            }
            let status = cmd.status().map_err(|_| Error::Git {
                message: format!("Failed to create worktree for {}", member.name),
            })?;
            if !status.success() {
                return Err(Error::Git {
                    message: format!("git worktree add failed for {}", member.name),
                });
            }
        }

        self.run_crumbly_index(path, verbose)?;

        Ok(())
    }

    fn run_crumbly_index(&self, path: &std::path::Path, verbose: bool) -> Result<(), Error> {
        let which_status = Command::new("which")
            .arg("crumbly")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if !matches!(which_status, Ok(s) if s.success()) {
            return Ok(());
        }

        let context_path = path.strip_prefix(&self.root).unwrap_or(path);
        let crumbly_dir = self.root.join(".crumbly");
        let subcommand = if crumbly_dir.exists() {
            "update"
        } else {
            "build"
        };

        println!(
            "{} crumbly index...",
            if subcommand == "build" {
                "Building"
            } else {
                "Updating"
            }
        );

        let mut cmd = Command::new("crumbly");
        cmd.args([subcommand, "--context", &context_path.display().to_string()])
            .current_dir(&self.root);
        if !verbose {
            cmd.stderr(Stdio::null());
        }

        let status = cmd.status().map_err(|_| Error::Git {
            message: "Failed to run crumbly".to_string(),
        })?;

        if !status.success() {
            return Err(Error::Git {
                message: format!("crumbly {} failed", subcommand),
            });
        }

        Ok(())
    }

    #[instrument(skip(self), err)]
    pub fn create_grove(
        &self,
        name: &str,
        branch: Option<&str>,
        verbose: bool,
    ) -> Result<(), Error> {
        let wt_dir = self.groves_dir().join(name);
        if wt_dir.exists() {
            println!("  {} grove '{}' already exists", "\u{2713}".green(), name);
            return Ok(());
        }

        std::fs::create_dir_all(&wt_dir).map_err(|e| Error::CreateDir {
            path: wt_dir.clone(),
            source: e,
        })?;

        let grove_marker = wt_dir.join(".grove");
        std::fs::create_dir_all(&grove_marker).map_err(|e| Error::CreateDir {
            path: grove_marker,
            source: e,
        })?;

        for member in &self.config.forest.member {
            let bare_path = self.bare_dir().join(format!("{}.git", member.name));
            let member_wt_path = wt_dir.join(&member.path);

            if member_wt_path.exists() {
                println!("  {} {}", "\u{2713}".green(), member.name.cyan());
                continue;
            }

            if let Some(parent) = member_wt_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| Error::CreateDir {
                    path: parent.to_path_buf(),
                    source: e,
                })?;
            }

            if verbose {
                println!("Creating worktree for {} in {}...", member.name, name);
            }

            let target_branch = branch.unwrap_or_else(|| member.branch());
            let use_new_branch = name != "develop" && branch.is_none();
            let new_branch_name = if use_new_branch {
                format!("{}/{}", name, member.name)
            } else {
                target_branch.to_string()
            };

            let status = if use_new_branch {
                let mut cmd = Command::new("git");
                cmd.args(["worktree", "add", "-b", &new_branch_name])
                    .arg(&member_wt_path)
                    .arg(member.branch())
                    .current_dir(&bare_path);
                if !verbose {
                    cmd.stdout(Stdio::null()).stderr(Stdio::null());
                }
                cmd.status().map_err(|_| Error::Git {
                    message: format!("Failed to create worktree for {}", member.name),
                })?
            } else {
                let mut cmd = Command::new("git");
                cmd.args(["worktree", "add"])
                    .arg(&member_wt_path)
                    .arg(target_branch)
                    .current_dir(&bare_path);
                if !verbose {
                    cmd.stdout(Stdio::null()).stderr(Stdio::null());
                }
                let status = cmd.status().map_err(|_| Error::Git {
                    message: format!("Failed to create worktree for {}", member.name),
                })?;

                if !status.success() {
                    let mut cmd = Command::new("git");
                    cmd.args(["worktree", "add", "-b", target_branch])
                        .arg(&member_wt_path)
                        .arg(member.branch())
                        .current_dir(&bare_path);
                    if !verbose {
                        cmd.stdout(Stdio::null()).stderr(Stdio::null());
                    }
                    cmd.status().map_err(|_| Error::Git {
                        message: format!("Failed to create worktree for {}", member.name),
                    })?
                } else {
                    status
                }
            };

            if !status.success() {
                return Err(Error::Git {
                    message: format!("git worktree add failed for {}", member.name),
                });
            }

            println!("  {} {}", "\u{2713}".green(), member.name.cyan());

            let mut cmd = Command::new("git");
            cmd.args(["remote", "remove", "origin"])
                .current_dir(&member_wt_path);
            if !verbose {
                cmd.stdout(Stdio::null()).stderr(Stdio::null());
            }
            cmd.status().map_err(|_| Error::Git {
                message: format!("Failed to remove origin remote for {}", member.name),
            })?;
        }

        if let Some(wt_config) = &self.config.grove {
            for symlink in &wt_config.symlink {
                let source = self.root.join(&symlink.source);
                let target = wt_dir.join(&symlink.target);
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| Error::CreateDir {
                        path: parent.to_path_buf(),
                        source: e,
                    })?;
                }
                std::os::unix::fs::symlink(&source, &target).map_err(|e| Error::Symlink {
                    src: source.clone(),
                    tgt: target.clone(),
                    source: e,
                })?;
                if verbose {
                    println!(
                        "Created symlink: {} -> {}",
                        target.display(),
                        source.display()
                    );
                }
            }
        }

        if verbose {
            println!("Updating crumbly index for {}...", name);
        }
        let context_path = format!("groves/{}", name);
        let mut cmd = Command::new("crumbly");
        cmd.args(["update", "--context", &context_path])
            .current_dir(&self.root);
        if !verbose {
            cmd.stdout(Stdio::null()).stderr(Stdio::null());
        }
        let _ = cmd.status();

        Ok(())
    }

    #[instrument(skip(self), err)]
    pub fn list_groves(&self) -> Result<Vec<String>, Error> {
        let wt_dir = self.groves_dir();
        if !wt_dir.exists() {
            return Ok(vec![]);
        }

        let mut groves = Vec::new();
        let entries = std::fs::read_dir(&wt_dir).map_err(|e| Error::CreateDir {
            path: wt_dir.clone(),
            source: e,
        })?;

        for entry in entries.flatten() {
            if entry.path().is_dir()
                && let Some(name) = entry.file_name().to_str()
            {
                groves.push(name.to_string());
            }
        }

        Ok(groves)
    }

    #[instrument(skip(self), err)]
    pub fn remove_grove(&self, name: &str, _force: bool) -> Result<(), Error> {
        let wt_dir = self.groves_dir().join(name);
        if !wt_dir.exists() {
            return Err(Error::GroveNotFound {
                name: name.to_string(),
            });
        }
        self.remove_grove_at(&wt_dir);
        Ok(())
    }

    fn remove_grove_at(&self, path: &PathBuf) {
        for member in &self.config.forest.member {
            let bare_path = self.bare_dir().join(format!("{}.git", member.name));
            let member_wt_path = path.join(&member.path);
            let _ = Command::new("git")
                .args(["worktree", "remove", "--force"])
                .arg(&member_wt_path)
                .current_dir(&bare_path)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        let _ = std::fs::remove_dir_all(path);
    }
}
