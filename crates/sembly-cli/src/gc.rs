//! Garbage collection command for the knowledge index.
//!
//! Removes orphaned chunks not referenced by any context's indexed files.

use clap::Parser;
use snafu::{ResultExt, Snafu};
use std::path::PathBuf;

#[allow(unused_imports)] // Used in format_gc_stats implementation
use crate::theme;
use sembly_core::knowledge::KnowledgeIndex;
use sembly_core::knowledge::facade::GcStats;

/// Arguments for garbage collection.
#[derive(Parser)]
pub struct GcArgs {
    /// Path to forest root (defaults to current directory)
    #[arg(long)]
    forest_root: Option<PathBuf>,
}

/// Runs garbage collection to remove orphaned chunks.
pub fn handle_gc(args: GcArgs) -> Result<(), GcError> {
    use gc_error::*;

    let forest_root = args
        .forest_root
        .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current directory"));

    let index = KnowledgeIndex::open(&forest_root).context(KnowledgeIndexSnafu)?;

    let stats = index.gc().context(KnowledgeIndexSnafu)?;

    format_gc_stats(&stats);

    Ok(())
}

/// Prints a formatted summary of garbage collection results.
fn format_gc_stats(stats: &GcStats) {
    println!(
        "{}",
        theme::success(format!(
            "Garbage collection complete: {} chunks deleted, {} embeddings deleted",
            stats.chunks_deleted, stats.embeddings_deleted
        ))
    );
}

#[derive(Debug, Snafu, miette::Diagnostic)]
#[snafu(module)]
pub enum GcError {
    #[snafu(display("Knowledge index operation failed"))]
    #[diagnostic(
        code(sembly::cli::gc_failed),
        help("Check the error details above for specific guidance")
    )]
    KnowledgeIndex {
        source: sembly_core::knowledge::facade::IndexError,
    },
}
