//! Knowledge indexing for Bottlerocket documentation
//!
//! This module provides semantic and keyword search across forest documentation,
//! enabling fast, targeted documentation lookup for AI agents and developers.
//!
//! # Architecture
//!
//! The knowledge index is organized into layers:
//!
//! * **Domain Layer** (`domain/`): Type-safe domain models
//!   Defines core concepts like `Chunk`, `SearchQuery`, and `IndexMode` (Fast/Best).
//!
//! * **Storage Layer** (`storage/`): Knowledgebase persistence
//!
//! * **Chunking Layer** (`chunking/`): Splits documentation into searchable chunks with
//!   context preservation (markdown headings, rustdoc items).
//!
//! * **Indexing Layer** (`indexing/`): File scanning and index building
//!
//! * **Search Layer** (`search/`): BM25 and semantic search engines
//!
//! * **Facade Layer** (`facade/`): High-level API via [`KnowledgeIndex`]
//!
//! # Index Modes
//!
//! - **Fast**: BM25 lexical search - quick keyword matching, no embeddings
//! - **Best**: Semantic search using all-MiniLM-L6-v2 embeddings - understands meaning
//!
//! # Quick Start
//!
//! ```no_run
//! use forester::knowledge::{KnowledgeIndex, IndexMode};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut index = KnowledgeIndex::open("/path/to/forest", IndexMode::Best)?;
//! let result = index.build()?;
//! let results = index.search("boot process", 10)?;
//! # Ok(())
//! # }
//! ```

pub mod chunking;
pub mod constants;
pub mod domain;
pub mod facade;
pub mod indexing;
pub mod search;
pub mod storage;

pub use chunking::{ChunkingError, ChunkingInput, ChunkingStrategy};
pub use domain::{
    Chunk, ChunkContent, ChunkContext, ChunkId, ChunkSource, ChunkableContent, Embedding,
    EmbeddingModelConfig, FileType, ForestRelativePath, IndexMetadata, IndexMode, IndexedChunk,
    LineRange, MarkdownContext, RepoName, RustDocContext, ScanConfig, SearchQuery, SearchResult,
    SearchResults, Timestamp,
};
pub use facade::{IndexError, IndexStatus, KnowledgeIndex};
pub use indexing::{
    FileScanner, ForesterConfig, ForesterConfigError, IndexResult, IndexStrategy, IndexableFile,
    Indexer, IndexingError, ScanError, load_forester_config,
};
pub use search::{SearchEngine, SearchError, SemanticSearchEngine};
pub use storage::{ChunkRepository, StorageError};
