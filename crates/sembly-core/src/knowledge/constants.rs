//! Constants for the knowledge indexing system.
//!
//! This module defines path constants for sembly's directory structure
//! and configuration constants for embedding and indexing behavior.

// Path constants

/// The sembly database directory name.
pub const SEMBLY_DIR: &str = ".sembly";

/// The sembly configuration file name.
pub const SEMBLY_CONFIG: &str = ".sembly.toml";

/// The sembly ignore file name.
pub const SEMBLY_IGNORE: &str = ".semblyignore";

/// The knowledge database filename.
pub const KNOWLEDGE_DB: &str = "knowledge.db";

/// The model cache subdirectory within SEMBLY_DIR.
pub const MODEL_CACHE_DIR: &str = "cache/model";

// Embedding and indexing configuration

/// Embedding vector dimension for all-MiniLM-L6-v2 model
pub const EMBEDDING_DIM: usize = 384;

/// Default tokenizer model for chunking and embeddings
pub const DEFAULT_TOKENIZER_MODEL: &str = "sentence-transformers/all-MiniLM-L6-v2";

/// Default maximum tokens per chunk (model's context window limit)
pub const DEFAULT_MAX_CHUNK_TOKENS: usize = 256;

/// Default overlap percentage between chunks (0.0 to 1.0)
pub const DEFAULT_CHUNK_OVERLAP_PERCENTAGE: f32 = 0.15;

/// Default computed overlap in tokens
pub const DEFAULT_CHUNK_OVERLAP_TOKENS: usize =
    (DEFAULT_MAX_CHUNK_TOKENS as f32 * DEFAULT_CHUNK_OVERLAP_PERCENTAGE) as usize;

/// Maximum number of threads for parallel file processing during indexing
///
/// Limits rayon parallelism to prevent CPU saturation. ONNX Runtime (used by
/// fastembed) also uses multiple threads per embedding generation, so limiting
/// file-level parallelism prevents thread oversubscription.
pub const MAX_INDEXING_THREADS: usize = 4;

/// Database schema version for multi-context indexing
///
/// Version 2 introduces content-addressed storage with contexts table,
/// modified indexed_files and chunks tables. Requires rebuild from version 1.
pub const SCHEMA_VERSION: u32 = 2;
