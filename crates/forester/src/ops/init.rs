//! Forest initialization operation.

use std::path::PathBuf;

use snafu::{ResultExt, Snafu};

use crate::events::{EventEmitter, ForesterEvent};
use crate::hooks::builtin::all_builtin_metas;

const GITIGNORE_ENTRIES: &[&str] = &[".forest/", "groves/"];

fn generate_template(name: &str) -> String {
    let mut s = format!(
        r#"[forest]
name = "{name}"

# [[forest.member]]
# name = "repo-name"
# remote = "https://github.com/org/repo.git"
# path = "repo-name"
# default-branch = "main"

# [grove]
# symlink = [
#   {{ source = "docs", target = "docs" }},
# ]
"#
    );

    for meta in all_builtin_metas() {
        s.push('\n');
        if meta.default_enabled {
            s.push_str(&format!("[[hook]]\nname = \"{}\"\n", meta.name));
        } else {
            s.push_str(&format!("# [[hook]]\n# name = \"{}\"\n", meta.name));
        }
    }

    s
}

fn update_gitignore(path: &PathBuf) -> Result<(), std::io::Error> {
    let existing = if path.exists() {
        std::fs::read_to_string(path)?
    } else {
        String::new()
    };

    let mut content = existing.clone();
    for entry in GITIGNORE_ENTRIES {
        if !existing.contains(entry) {
            if !content.is_empty() && !content.ends_with('\n') {
                content.push('\n');
            }
            content.push_str(entry);
            content.push('\n');
        }
    }

    std::fs::write(path, content)
}

/// Initializes a new forest with a template config.
pub struct InitOperation<'a> {
    path: PathBuf,
    name: Option<String>,
    emitter: &'a dyn EventEmitter,
}

impl<'a> InitOperation<'a> {
    /// Creates a new init operation.
    pub fn new(path: PathBuf, name: Option<String>, emitter: &'a dyn EventEmitter) -> Self {
        Self {
            path,
            name,
            emitter,
        }
    }

    /// Executes the initialization.
    pub fn execute(&self) -> Result<PathBuf, InitError> {
        use init_error::*;

        let config_path = self.path.join("forester.toml");

        if config_path.exists() {
            return Err(InitError::AlreadyExists { path: config_path });
        }

        let name = self.name.clone().unwrap_or_else(|| {
            self.path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("my-forest")
                .to_string()
        });

        std::fs::create_dir_all(&self.path).context(CreateDirSnafu { path: &self.path })?;
        std::fs::write(&config_path, generate_template(&name))
            .context(WriteSnafu { path: &config_path })?;

        let gitignore_path = self.path.join(".gitignore");
        update_gitignore(&gitignore_path).context(WriteSnafu {
            path: &gitignore_path,
        })?;

        self.emitter.emit(&ForesterEvent::Info(format!(
            "Created {}",
            config_path.display()
        )));

        Ok(config_path)
    }
}

/// Errors from initializing a forest.
#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum InitError {
    /// Config file already exists.
    #[snafu(display("Config already exists at {}", path.display()))]
    AlreadyExists {
        /// Path to existing config.
        path: PathBuf,
    },

    /// Failed to create a directory.
    #[snafu(display("Failed to create directory"))]
    CreateDir {
        /// Path that could not be created.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },

    /// Failed to write the config file.
    #[snafu(display("Failed to write config"))]
    Write {
        /// Path that could not be written.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
}
