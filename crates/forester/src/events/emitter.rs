//! Event emission for forester operations.

use owo_colors::OwoColorize;

use super::ForesterEvent;

/// Trait for emitting forester events.
pub trait EventEmitter {
    /// Emit an event.
    fn emit(&self, event: &ForesterEvent);
}

/// Console-based event emitter with color support.
#[derive(Debug, Clone)]
pub struct ConsoleEmitter {
    verbose: bool,
}

impl ConsoleEmitter {
    /// Creates a new console emitter.
    pub fn new(verbose: bool) -> Self {
        Self { verbose }
    }
}

impl EventEmitter for ConsoleEmitter {
    fn emit(&self, event: &ForesterEvent) {
        match event {
            ForesterEvent::SeedStarted => {
                eprintln!("{} Seeding forest...", "→".cyan());
            }
            ForesterEvent::MemberCloning { name, remote } => {
                if self.verbose {
                    eprintln!("  {} {} from {}", "cloning".blue(), name, remote.dimmed());
                } else {
                    eprintln!("  {} {}", "cloning".blue(), name);
                }
            }
            ForesterEvent::MemberCloned { name } => {
                if self.verbose {
                    eprintln!("  {} {}", "cloned".green(), name);
                }
            }
            ForesterEvent::SeedCompleted => {
                eprintln!("{} Forest seeded", "✓".green());
            }
            ForesterEvent::GroveCreating { name } => {
                eprintln!("{} Creating grove '{}'", "→".cyan(), name);
            }
            ForesterEvent::GroveWorktreeCreated { member, path } => {
                if self.verbose {
                    eprintln!(
                        "  {} {} at {}",
                        "worktree".blue(),
                        member,
                        path.display().to_string().dimmed()
                    );
                }
            }
            ForesterEvent::GroveCreated { name } => {
                eprintln!("{} Grove '{}' created", "✓".green(), name);
            }
            ForesterEvent::GroveRemoving { name } => {
                eprintln!("{} Removing grove '{}'", "→".cyan(), name);
            }
            ForesterEvent::GroveRemoved { name } => {
                eprintln!("{} Grove '{}' removed", "✓".green(), name);
            }
            ForesterEvent::GroveListing => {
                if self.verbose {
                    eprintln!("{} Listing groves", "→".cyan());
                }
            }
            ForesterEvent::HookExecuting { name } => {
                eprintln!("  {} hook '{}'", "running".blue(), name);
            }
            ForesterEvent::HookCompleted { name } => {
                if self.verbose {
                    eprintln!("  {} hook '{}'", "completed".green(), name);
                }
            }
            ForesterEvent::Warning(msg) => {
                eprintln!("{} {}", "warning:".yellow(), msg);
            }
            ForesterEvent::Info(msg) => {
                eprintln!("{} {}", "info:".blue(), msg);
            }
        }
    }
}
