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
#![allow(dead_code)]
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

use owo_colors::{OwoColorize, Stream, Style};

/// Helper to apply a style only when colors are supported
fn styled(s: impl std::fmt::Display, style: Style) -> String {
    s.if_supports_color(Stream::Stdout, |s| s.style(style))
        .to_string()
}

/// Success indicators (checkmarks, "Running", etc.)
pub fn success(s: impl std::fmt::Display) -> String {
    styled(s, Style::new().bright_green().bold())
}

/// Error states and removals
pub fn error(s: impl std::fmt::Display) -> String {
    styled(s, Style::new().bright_red().bold())
}

/// Warnings and intermediate states
pub fn warning(s: impl std::fmt::Display) -> String {
    styled(s, Style::new().yellow())
}

/// Values, counts, and data (cyan for consistency with clap's LITERAL style)
pub fn value(s: impl std::fmt::Display) -> String {
    styled(s, Style::new().cyan())
}

/// URLs and links
pub fn url(s: impl std::fmt::Display) -> String {
    styled(s, Style::new().cyan().underline())
}

/// Labels and repository names
pub fn label(s: impl std::fmt::Display) -> String {
    styled(s, Style::new().bright_white())
}

/// Secondary information and metadata
pub fn muted(s: impl std::fmt::Display) -> String {
    styled(s, Style::new().dimmed())
}

/// Emphasized muted text (italic + dimmed)
pub fn muted_italic(s: impl std::fmt::Display) -> String {
    styled(s, Style::new().dimmed().italic())
}

/// Tags and identifiers
pub fn tag(s: impl std::fmt::Display) -> String {
    styled(s, Style::new().blue())
}

/// Sizes and measurements
pub fn size(s: impl std::fmt::Display) -> String {
    styled(s, Style::new().yellow())
}

/// Timestamps and time-related info
pub fn time(s: impl std::fmt::Display) -> String {
    styled(s, Style::new().green())
}

/// Score colorization based on relevance thresholds
///
/// Colors automatically disable when output is not a TTY.
pub fn score(value: f32) -> String {
    let formatted = format!("{:.3}", value);
    let style = if value >= 0.8 {
        Style::new().green()
    } else if value >= 0.6 {
        Style::new().yellow()
    } else if value >= 0.4 {
        Style::new().truecolor(255, 165, 0)
    } else {
        Style::new().red()
    };
    styled(formatted, style)
}

/// Progress bar template strings matching the theme
///
/// These templates use indicatif's inline color syntax and should be used
/// with `ProgressStyle::default_spinner().template()` or `ProgressStyle::default_bar().template()`.
pub mod progress {
    pub const SCAN: &str =
        "[{elapsed_precise:.dimmed}] {spinner:.green} {msg:<11.blue} ({pos:>4.cyan} files found)";
    pub const CHUNK: &str = "[{elapsed_precise:.dimmed}]   {msg:<11.blue} [{bar:40.cyan/blue}] ({pos:>4.cyan}/{len:<4.bold.cyan} files)";
    pub const EMBED: &str = "[{elapsed_precise:.dimmed}] {spinner:.green} {msg:<11.blue} ({pos:>4.cyan}/{len:<4.bold.cyan} chunks indexed)";
}
