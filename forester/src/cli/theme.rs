//! Centralized color theme for CLI output
//!
//! Provides consistent styling across all CLI commands, matching the aesthetic
//! of clap-cargo's help output.
//!
//! # Usage
//!
//! ```ignore
//! use super::theme;
//!
//! println!("{} Operation successful", theme::success("✓"));
//! println!("  URL: {}", theme::url("http://localhost:5000"));
//! println!("  Count: {}", theme::value(42));
//! ```
//!
//! # Design
//!
//! This module uses `owo_colors` for terminal output while maintaining visual
//! consistency with `clap-cargo`'s `anstyle`-based help styling. The color
//! palette is chosen to match the bright, bold aesthetic of cargo-style CLIs.

use owo_colors::OwoColorize;

/// Success indicators (checkmarks, "Running", etc.)
pub fn success(s: impl std::fmt::Display) -> String {
    s.to_string().bright_green().bold().to_string()
}

/// Error states and removals
pub fn error(s: impl std::fmt::Display) -> String {
    s.to_string().bright_red().bold().to_string()
}

/// Warnings and intermediate states
pub fn warning(s: impl std::fmt::Display) -> String {
    s.to_string().yellow().to_string()
}

/// Values, counts, and data (cyan for consistency with clap's LITERAL style)
pub fn value(s: impl std::fmt::Display) -> String {
    s.to_string().cyan().to_string()
}

/// URLs and links
pub fn url(s: impl std::fmt::Display) -> String {
    s.to_string().cyan().underline().to_string()
}

/// Labels and repository names
pub fn label(s: impl std::fmt::Display) -> String {
    s.to_string().bright_white().to_string()
}

/// Secondary information and metadata
pub fn muted(s: impl std::fmt::Display) -> String {
    s.to_string().dimmed().to_string()
}

/// Emphasized muted text (italic + dimmed)
pub fn muted_italic(s: impl std::fmt::Display) -> String {
    s.to_string().dimmed().italic().to_string()
}

/// Tags and identifiers
pub fn tag(s: impl std::fmt::Display) -> String {
    s.to_string().blue().to_string()
}

/// Sizes and measurements
pub fn size(s: impl std::fmt::Display) -> String {
    s.to_string().yellow().to_string()
}

/// Timestamps and time-related info
pub fn time(s: impl std::fmt::Display) -> String {
    s.to_string().green().to_string()
}

/// Score colorization based on relevance thresholds
///
/// Colors automatically disable when output is not a TTY.
pub fn score(value: f32) -> String {
    let formatted = format!("{:.3}", value);
    if value >= 0.8 {
        formatted.green().to_string()
    } else if value >= 0.6 {
        formatted.yellow().to_string()
    } else if value >= 0.4 {
        formatted.truecolor(255, 165, 0).to_string() // orange
    } else {
        formatted.red().to_string()
    }
}

/// Progress bar template strings matching the theme
///
/// These templates use indicatif's inline color syntax and should be used
/// with `ProgressStyle::default_spinner().template()`.
pub mod progress {
    pub const SCAN: &str =
        "[{elapsed_precise:.dimmed}] {spinner:.green} {msg:.blue} ({pos:.cyan} files found)";
    pub const CHUNK: &str = "[{elapsed_precise:.dimmed}] {spinner:.green} {msg:.blue} ({pos:.cyan}/{len:.bold.cyan} files)";
    pub const EMBED: &str = "[{elapsed_precise:.dimmed}] {spinner:.green} {msg:.blue} ({pos:.cyan}/{len:.bold.cyan} chunks indexed)";
}
