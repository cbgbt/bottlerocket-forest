//! Indexing orchestration for building and maintaining the knowledge index

pub mod scanner;

pub use scanner::{FileScanner, IndexableFile, ScanError};
