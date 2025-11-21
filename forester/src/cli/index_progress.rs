//! CLI progress reporter using indicatif
//!
//! Provides terminal-based progress bars for index building operations.
//! Displays three concurrent progress bars:
//! - Scan bar: File discovery (spinner → final count)
//! - Chunk bar: File chunking progress
//! - Embed bar: Embedding generation progress

use crate::knowledge::indexing::ProgressReporter;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::path::Path;
use std::sync::Arc;

/// CLI progress reporter with three concurrent progress bars
///
/// Displays real-time progress for scanning, chunking, and embedding phases
/// using indicatif. Thread-safe and suitable for parallel processing.
pub struct CliProgressReporter {
    #[allow(dead_code)]
    multi: Arc<MultiProgress>,
    scan_bar: Arc<ProgressBar>,
    chunk_bar: Arc<ProgressBar>,
    embed_bar: Arc<ProgressBar>,
}

impl CliProgressReporter {
    /// Create a new CLI progress reporter with default styles
    ///
    /// Sets up three progress bars:
    /// - Scan: Spinner showing file discovery
    /// - Chunk: Progress bar for chunking files
    /// - Embed: Spinner showing indexing
    pub fn new() -> Self {
        Self::with_styles(
            Self::default_scan_style(),
            Self::default_chunk_style(),
            Self::default_embed_style(),
        )
    }

    /// Create with custom progress bar styles
    ///
    /// Allows full customization of progress bar appearance. Useful for
    /// matching specific terminal themes or output requirements.
    pub fn with_styles(
        scan_style: ProgressStyle,
        chunk_style: ProgressStyle,
        embed_style: ProgressStyle,
    ) -> Self {
        let multi = Arc::new(MultiProgress::new());

        let scan_bar = Arc::new(multi.add(ProgressBar::new_spinner()));
        scan_bar.set_style(scan_style);

        let chunk_bar = Arc::new(multi.add(ProgressBar::new(0)));
        chunk_bar.set_style(chunk_style);

        let embed_bar = Arc::new(multi.add(ProgressBar::new(0)));
        embed_bar.set_style(embed_style);

        Self {
            multi,
            scan_bar,
            chunk_bar,
            embed_bar,
        }
    }

    /// Get default scan progress style
    ///
    /// Returns a spinner style for the scanning phase:
    /// `⠋ Scanning... (123 files found)`
    fn default_scan_style() -> ProgressStyle {
        ProgressStyle::default_spinner()
            .template("{spinner:.green} Scanning... ({pos} files found)")
            .unwrap()
    }

    /// Get default chunk progress style
    ///
    /// Returns a progress bar style for chunking:
    /// `Chunking [████████░░] 45/100 files`
    fn default_chunk_style() -> ProgressStyle {
        ProgressStyle::default_bar()
            .template("Chunking [{bar:40.cyan/blue}] {pos}/{len} files")
            .unwrap()
            .progress_chars("█░")
    }

    /// Get default embed progress style
    ///
    /// Returns a progress bar style for embedding:
    /// `⠋ Embedding (450 chunks)`
    fn default_embed_style() -> ProgressStyle {
        ProgressStyle::default_spinner()
            .template("{spinner:.green} Embedding... ({pos} chunks)")
            .unwrap()
    }
}

impl Default for CliProgressReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressReporter for CliProgressReporter {
    fn scanning_started(&self) {
        self.scan_bar.set_position(0);
        self.scan_bar
            .enable_steady_tick(std::time::Duration::from_millis(100));
    }

    fn file_discovered(&self, _path: &Path) {
        self.scan_bar.inc(1);
    }

    fn scanning_completed(&self, total_files: usize) {
        self.scan_bar
            .finish_with_message(format!("Scanned {} files", total_files));
    }

    fn chunking_started(&self, total_files: usize) {
        self.chunk_bar.set_length(total_files as u64);
        self.chunk_bar.set_position(0);
    }

    fn file_chunked(&self, _path: &Path, _chunk_count: usize) {
        self.chunk_bar.inc(1);
    }

    fn chunking_completed(&self, total_chunks: usize) {
        self.chunk_bar
            .finish_with_message(format!("Chunked into {} chunks", total_chunks));
    }

    fn embedding_started(&self, total_chunks: usize) {
        self.embed_bar.set_length(total_chunks as u64);
        self.embed_bar.set_position(0);
    }

    fn embeddings_generated(&self, chunk_count: usize) {
        self.embed_bar.inc(chunk_count as u64);
    }

    fn embedding_completed(&self) {
        self.embed_bar.finish_with_message("Embeddings generated");
    }

    fn indexing_completed(&self) {
        // All progress bars are already finished, nothing to do
    }
}
